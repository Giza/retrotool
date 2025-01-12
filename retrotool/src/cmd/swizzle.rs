use std::path::PathBuf;
use anyhow::Result;
use argh::FromArgs;
use ddsfile::{Dds, DxgiFormat};
use retrolib::format::txtr::{
    STextureHeader, ETextureFormat, ETextureType, STextureSamplerData,
    ETextureFilter, ETextureWrap, ETextureAnisotropicRatio, ETextureMipFilter
};

#[derive(FromArgs, PartialEq, Debug)]
/// Swizzle изображения
#[argh(subcommand, name = "swizzle")]
pub struct Args {
    /// путь к входному файлу
    #[argh(option, short = 'i')]
    input: PathBuf,

    /// путь к выходному файлу
    #[argh(option, short = 'o')]
    output: PathBuf,

    /// ширина изображения (необязательно если входной файл - DDS)
    #[argh(option, short = 'w')]
    width: Option<u32>,

    /// высота изображения (необязательно если входной файл - DDS)
    #[argh(option, short = 'h')]
    height: Option<u32>,

    /// формат текстуры (необязательно если входной файл - DDS)
    #[argh(option, short = 'f')]
    format: Option<String>,

    /// размер mip-уровня в байтах
    #[argh(option, short = 'm')]
    mip_size: u32,
}

fn dxgi_format_to_texture_format(format: DxgiFormat) -> Option<ETextureFormat> {
    match format {
        DxgiFormat::R8G8B8A8_UNorm => Some(ETextureFormat::Rgba8Unorm),
        DxgiFormat::R8_UNorm => Some(ETextureFormat::R8Unorm),
        DxgiFormat::BC7_UNorm | DxgiFormat::BC7_UNorm_sRGB => Some(ETextureFormat::BptcUnormSrgb),
        // Добавьте другие форматы по необходимости
        _ => None,
    }
}

pub fn run(args: Args) -> Result<()> {
    // Читаем входной файл
    let input_data = std::fs::read(&args.input)?;
    
    // Пытаемся прочитать как DDS, если расширение .dds
    let (width, height, format, texture_data) = if args.input.extension().and_then(|ext| ext.to_str()) == Some("dds") {
        match Dds::read(&mut std::io::Cursor::new(&input_data)) {
            Ok(dds) => {
                let width = dds.get_width();
                let height = dds.get_height();
                let dxgi_format = dds.get_dxgi_format().ok_or_else(|| anyhow::anyhow!("Формат DDS не поддерживается"))?;
                
                log::info!("Чтение DDS файла:");
                log::info!("Ширина: {}", width);
                log::info!("Высота: {}", height);
                log::info!("DDS формат: {:?}", dxgi_format);
                
                let texture_format = dxgi_format_to_texture_format(dxgi_format)
                    .ok_or_else(|| anyhow::anyhow!("Неподдерживаемый DDS формат: {:?}", dxgi_format))?;
                
                // Получаем данные первого mip-уровня
                let data = dds.get_data(0)
                    .map_err(|e| anyhow::anyhow!("Не удалось получить данные текстуры: {}", e))?;
                
                log::info!("Размер данных текстуры: {} байт", data.len());
                
                (width, height, texture_format, data.to_vec())
            }
            Err(e) => {
                log::warn!("Не удалось прочитать DDS файл: {}. Используем параметры командной строки", e);
                (
                    args.width.ok_or_else(|| anyhow::anyhow!("Не указана ширина"))?,
                    args.height.ok_or_else(|| anyhow::anyhow!("Не указана высота"))?,
                    match args.format.as_ref().ok_or_else(|| anyhow::anyhow!("Не указан формат"))?.to_uppercase().as_str() {
                        "R8" => ETextureFormat::R8Unorm,
                        "RGBA8" => ETextureFormat::Rgba8Unorm,
                        "BC7" | "BPTC" => ETextureFormat::BptcUnormSrgb,
                        _ => anyhow::bail!("Неподдерживаемый формат текстуры: {}. Поддерживаемые форматы: R8, RGBA8, BC7", args.format.unwrap()),
                    },
                    input_data
                )
            }
        }
    } else {
        // Если не DDS, используем параметры командной строки
        (
            args.width.ok_or_else(|| anyhow::anyhow!("Не указана ширина"))?,
            args.height.ok_or_else(|| anyhow::anyhow!("Не указана высота"))?,
            match args.format.as_ref().ok_or_else(|| anyhow::anyhow!("Не указан формат"))?.to_uppercase().as_str() {
                "R8" => ETextureFormat::R8Unorm,
                "RGBA8" => ETextureFormat::Rgba8Unorm,
                "BC7" | "BPTC" => ETextureFormat::BptcUnormSrgb,
                _ => anyhow::bail!("Неподдерживаемый формат текстуры: {}. Поддерживаемые форматы: R8, RGBA8, BC7", args.format.unwrap()),
            },
            input_data
        )
    };

    // Создаем заголовок для получения параметров
    let header = STextureHeader {
        width,
        height,
        layers: 1,
        format,
        kind: ETextureType::D2,
        mip_sizes: vec![args.mip_size],
        tile_mode: 0,
        swizzle: 0,
        sampler_data: STextureSamplerData {
            unk: 0,
            filter: ETextureFilter::Linear,
            wrap_x: ETextureWrap::Repeat,
            wrap_y: ETextureWrap::Repeat,
            wrap_z: ETextureWrap::Repeat,
            mip_filter: ETextureMipFilter::Linear,
            aniso: ETextureAnisotropicRatio::Ratio1,
        },
    };

    // Логируем параметры текстуры
    log::info!("Параметры текстуры:");
    log::info!("Ширина: {}", width);
    log::info!("Высота: {}", height);
    log::info!("Формат: {:?}", format);
    log::info!("Байт на пиксель: {}", format.bytes_per_pixel());
    log::info!("Размер блока: {:?}", format.block_size());
    log::info!("Слои: {}", header.layers);
    log::info!("Тип: {:?}", header.kind);
    log::info!("Размер mip-уровня: {} байт", args.mip_size);
    
    let expected_size = args.mip_size as usize;
    log::info!("Размер входных данных: {} байт", texture_data.len());

    // Проверяем размер входных данных
    if texture_data.len() != expected_size {
        log::warn!("Размер входных данных не соответствует ожидаемому");
        log::warn!("Попытка подогнать размер...");
        let mut adjusted_data = texture_data.clone();
        if adjusted_data.len() < expected_size {
            adjusted_data.resize(expected_size, 0);
        } else {
            adjusted_data.truncate(expected_size);
        }
        log::info!("Размер скорректирован до {} байт", adjusted_data.len());

        let swizzled = retrolib::format::txtr::swizzle(&header, &adjusted_data)?;
        std::fs::write(&args.output, swizzled)?;
    } else {
        let swizzled = retrolib::format::txtr::swizzle(&header, &texture_data)?;
        std::fs::write(&args.output, swizzled)?;
    }

    log::info!("Swizzle выполнен успешно");
    log::info!("Входной файл: {}", args.input.display());
    log::info!("Выходной файл: {}", args.output.display());
    log::info!("Размеры: {}x{}", width, height);

    Ok(())
} 