use egui_extras::RetainedImage;

pub(crate) struct SiftImage {
    pub(crate) image: RetainedImage,
}

impl SiftImage {
    pub(crate) fn new<S: Into<String>>(name: S, image: egui::ColorImage) -> SiftImage {
        SiftImage {
            image: RetainedImage::from_color_image(name, image),
        }
    }

    pub(crate) fn from_path<S: Into<String>, P: AsRef<std::path::Path>>(
        name: S,
        path: P,
    ) -> anyhow::Result<SiftImage> {
        let image = load_image_from_path(path.as_ref())?;
        Ok(SiftImage::new(name, image))
    }

    // pub(crate) fn from_bytes<S: Into<String>>(name: S, bytes: &[u8]) -> anyhow::Result<SiftImage> {
    //     let image = load_image_from_memory(bytes)?;
    //     Ok(SiftImage::new(name, image))
    // }
}

fn load_image_from_path(path: &std::path::Path) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::io::Reader::open(path)?.decode()?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

// fn load_image_from_memory(image_data: &[u8]) -> Result<egui::ColorImage, image::ImageError> {
//     let image = image::load_from_memory(image_data)?;
//     let size = [image.width() as _, image.height() as _];
//     let image_buffer = image.to_rgba8();
//     let pixels = image_buffer.as_flat_samples();
//     Ok(egui::ColorImage::from_rgba_unmultiplied(
//         size,
//         pixels.as_slice(),
//     ))
// }
