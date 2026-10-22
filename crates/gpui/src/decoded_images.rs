//! Decoded pictures `img` keeps loaded, held to a byte budget once an app sets one: past it,
//! the least recently drawn pictures that no window shows are let go, their atlas tiles with
//! them, and decode again if they are drawn later.

use std::any::TypeId;
use std::sync::Arc;

use collections::{FxHashSet, IndexMap};

use crate::{App, ImageId, RenderImage, Window};

#[derive(Default)]
pub(crate) struct DecodedImages {
    /// By asset key, least recently drawn first.
    held: IndexMap<(TypeId, u64), Arc<RenderImage>>,
    bytes: usize,
    budget: Option<usize>,
}

fn size_of(image: &RenderImage) -> usize {
    (0..image.frame_count())
        .filter_map(|frame| image.as_bytes(frame))
        .map(<[u8]>::len)
        .sum()
}

impl DecodedImages {
    pub(crate) fn drawn(&mut self, key: (TypeId, u64), image: &Arc<RenderImage>) {
        if self.budget.is_none() {
            return;
        }
        match self.held.shift_remove(&key) {
            Some(held) => {
                self.held.insert(key, held);
            }
            None => {
                self.bytes += size_of(image);
                self.held.insert(key, image.clone());
            }
        }
    }
}

impl App {
    /// Holds decoded pictures to `bytes` in all: once a frame leaves more than that loaded,
    /// the least recently drawn that no window shows are dropped until it fits.
    pub fn set_decoded_image_budget(&mut self, bytes: usize) {
        self.decoded_images.budget = Some(bytes);
    }

    pub(crate) fn decoded_image_drawn<A: crate::Asset>(
        &mut self,
        source: &A::Source,
        image: &Arc<RenderImage>,
    ) {
        let key = (TypeId::of::<A>(), crate::hash(source));
        self.decoded_images.drawn(key, image);
    }

    /// Lets go of pictures past the budget, once `current` has drawn its frame.
    pub(crate) fn trim_decoded_images(&mut self, current: &mut Window) {
        let Some(budget) = self.decoded_images.budget else {
            return;
        };
        if self.decoded_images.bytes <= budget {
            return;
        }
        let mut shown: FxHashSet<ImageId> = current.shown_images();
        for window in self.windows.values().flatten() {
            shown.extend(window.shown_images());
        }
        let mut bytes = self.decoded_images.bytes;
        let mut dropped = Vec::new();
        self.decoded_images.held.retain(|key, image| {
            if bytes <= budget || shown.contains(&image.id) {
                return true;
            }
            bytes -= size_of(image);
            dropped.push((*key, image.clone()));
            false
        });
        self.decoded_images.bytes = bytes;
        for (key, image) in dropped {
            self.loading_assets.remove(&key);
            self.drop_image(image, Some(current));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;

    use crate::{
        Context, Image, ImageFormat, ImageSource, IntoElement, ParentElement as _, Render,
        Styled as _, TestAppContext, Window, div, img, px,
    };

    /// A 16 by 16 picture, 1024 bytes once decoded, in a colour no other one shares.
    fn picture(shade: u8) -> Arc<Image> {
        let pixels = image::RgbaImage::from_pixel(16, 16, image::Rgba([shade, 0, 0, 255]));
        let mut png = Vec::new();
        pixels
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("a png");
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    }

    struct Pictures(Vec<Arc<Image>>);

    impl Render for Pictures {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().children(
                self.0
                    .iter()
                    .map(|picture| img(ImageSource::Image(picture.clone())).size(px(16.))),
            )
        }
    }

    #[crate::test]
    fn a_picture_no_longer_drawn_is_let_go_past_the_budget(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_decoded_image_budget(1500));
        let (first, second) = (picture(1), picture(2));
        let (view, cx) = cx.add_window_view(|_, _| Pictures(vec![first.clone()]));
        cx.run_until_parked();
        assert!(cx.update(|_, cx| ImageSource::Image(first.clone()).is_asset_cached(cx)));

        view.update(cx, |pictures, cx| {
            pictures.0 = vec![second.clone()];
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(!ImageSource::Image(first.clone()).is_asset_cached(cx));
            assert!(ImageSource::Image(second.clone()).is_asset_cached(cx));
        });
    }

    #[crate::test]
    fn a_picture_on_screen_is_kept_over_the_budget(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_decoded_image_budget(0));
        let (first, second) = (picture(1), picture(2));
        let (view, cx) = cx.add_window_view(|_, _| Pictures(vec![first.clone(), second.clone()]));
        cx.run_until_parked();
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(ImageSource::Image(first.clone()).is_asset_cached(cx));
            assert!(ImageSource::Image(second.clone()).is_asset_cached(cx));
        });
    }
}
