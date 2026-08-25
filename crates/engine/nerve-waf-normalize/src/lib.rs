//! HTTP parameter locate + recursive decode for inspection.

mod decode;
mod locate;

pub use decode::{
    decode_chain, is_static_asset_path, looks_suspicious, DecodeConfig, DecodeLayer, DecodePolicy,
};
pub use locate::{
    locate_params, locate_params_scoped, DetectionScope, ParamKind, ParamSlice,
};

#[cfg(test)]
mod test;
