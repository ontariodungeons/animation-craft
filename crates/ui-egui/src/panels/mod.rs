//! Panels module.

pub mod center;
pub mod header;
pub mod left;
pub mod right;
pub(crate) mod agent;
pub(crate) mod start;
pub(crate) mod workshop;

pub(super) mod chrome;

pub(super) mod asset_browser;

pub(crate) use center::pose_viewer::sheet_path;
