//! Asset source: the vendored Fluent icons plus gpui-component's own bundle.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

use crate::icons_data::ICONS;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let debug = std::env::var_os("OK_GPUI_ASSET_DEBUG").is_some();
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            if debug {
                eprintln!("asset hit {path} ({} bytes)", bytes.len());
            }
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        let result = gpui_component_assets::Assets.load(path);
        if debug {
            eprintln!(
                "asset delegate {path} -> {}",
                match result.as_ref() {
                    Ok(Some(bytes)) => format!("{} bytes", bytes.len()),
                    Ok(None) => "miss".to_owned(),
                    Err(error) => format!("error {error}"),
                }
            );
        }
        result
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let owned: Vec<SharedString> = ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect();
        if !owned.is_empty() {
            return Ok(owned);
        }
        gpui_component_assets::Assets.list(path)
    }
}
