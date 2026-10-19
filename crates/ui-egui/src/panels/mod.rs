//! Panels module.

pub mod center;
pub mod header;
pub mod left;
pub mod right;
pub(crate) mod start;

pub(super) mod chrome;

pub(super) mod asset_browser;

pub(crate) use center::pose_viewer::sheet_path;
