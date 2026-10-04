//! The renderer reads only bytes that passed the checks (docs/DECISIONS.md, D16; invariant I11).
//!
//! Bevy's default asset source is replaced by [`CheckedAssetReader`], which serves the files
//! registered in [`CheckedAssets`] from memory and refuses every other read:
//!
//! * the map, its external tileset, and its tileset image reach `bevy_ecs_tiled` and the image
//!   decoder as exactly the bytes that `sim::load_map` and `crate::tileset_image` checked, and are
//!   not read from disk a second time (M1 review 1, P2-1; M1 review 2, P2-C), so a file changed
//!   after its check cannot reach the renderer;
//! * no `.meta` file is ever read: Bevy otherwise reads `<asset>.meta` whole and without a size
//!   limit for every asset it loads and lets it choose the loader and its settings, which is an
//!   unchecked external file;
//! * a load of any other file fails, which `crate::map::exit_on_asset_load_failure` turns into
//!   exit code 1.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, VecReader,
};
use bevy::prelude::*;

/// The files the renderer may read, keyed by their path in the asset directory, with the bytes
/// that were checked. Shared with the asset reader.
#[derive(Resource, Clone, Default)]
pub struct CheckedAssets(Arc<RwLock<BTreeMap<PathBuf, Arc<[u8]>>>>);

impl CheckedAssets {
    /// Registers checked `bytes` as the content of `asset_path`.
    pub fn insert(&self, asset_path: impl Into<PathBuf>, bytes: Vec<u8>) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(asset_path.into(), bytes.into());
    }

    fn get(&self, asset_path: &Path) -> Option<Arc<[u8]>> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(asset_path)
            .cloned()
    }
}

/// The default asset source's reader: see the module documentation.
pub struct CheckedAssetReader(pub CheckedAssets);

impl AssetReader for CheckedAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        self.0
            .get(path)
            .map(|bytes| VecReader::new(bytes.to_vec()))
            .ok_or_else(|| AssetReaderError::NotFound(path.to_owned()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        // "Not found" makes Bevy use the loader's default settings.
        Err(AssetReaderError::NotFound(path.to_owned()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_owned()))
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}

/// Makes [`CheckedAssetReader`] the default asset source and inserts the shared
/// [`CheckedAssets`]. It must be added before Bevy's `AssetPlugin`, which builds the sources.
pub struct CheckedAssetsPlugin;

impl Plugin for CheckedAssetsPlugin {
    fn build(&self, app: &mut App) {
        assert!(
            !app.world().contains_resource::<AssetServer>(),
            "CheckedAssetsPlugin must be added before AssetPlugin"
        );
        let assets = CheckedAssets::default();
        let shared = assets.clone();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(CheckedAssetReader(shared.clone()))),
        )
        .insert_resource(assets);
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::io::Reader;
    use bevy::tasks::block_on;

    use super::*;

    fn read(reader: &CheckedAssetReader, path: &str) -> Result<Vec<u8>, AssetReaderError> {
        block_on(async {
            let mut source = reader.read(Path::new(path)).await?;
            let mut bytes = Vec::new();
            source
                .read_to_end(&mut bytes)
                .await
                .map_err(|e| AssetReaderError::Io(Arc::new(e)))?;
            Ok(bytes)
        })
    }

    #[test]
    fn only_registered_bytes_are_served_and_meta_files_are_never_read() {
        let assets = CheckedAssets::default();
        assets.insert("maps/demo/demo.tmx", b"checked".to_vec());
        let reader = CheckedAssetReader(assets.clone());
        assert_eq!(
            read(&reader, "maps/demo/demo.tmx").expect("registered"),
            b"checked"
        );
        // A file that exists on disk but was not checked is not served.
        assert!(matches!(
            read(&reader, "maps/demo/demo_tiles.png"),
            Err(AssetReaderError::NotFound(_))
        ));
        // `.meta` files are never read, not even for a registered asset.
        assert!(matches!(
            block_on(reader.read_meta(Path::new("maps/demo/demo.tmx"))),
            Err(AssetReaderError::NotFound(_))
        ));
    }

    #[test]
    fn the_plugin_replaces_the_default_asset_source() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, CheckedAssetsPlugin, AssetPlugin::default()));
        app.world()
            .resource::<CheckedAssets>()
            .insert("maps/demo/demo.tmx", b"checked".to_vec());
        let server = app.world().resource::<AssetServer>();
        let source = server
            .get_source(AssetSourceId::Default)
            .expect("default source");
        let bytes = block_on(async {
            let mut reader = source
                .reader()
                .read(Path::new("maps/demo/demo.tmx"))
                .await?;
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .await
                .map_err(|e| AssetReaderError::Io(Arc::new(e)))?;
            Ok::<_, AssetReaderError>(bytes)
        })
        .expect("served from the checked assets");
        assert_eq!(bytes, b"checked");
        // The demo map's tileset image exists on disk under the default asset directory, but it
        // was not registered, so the default source does not serve it.
        assert!(block_on(source.reader().read(Path::new("maps/demo/demo_tiles.png"))).is_err());
    }
}
