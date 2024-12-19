use clap::ValueEnum;

#[derive(Debug, Clone, ValueEnum)]
pub enum DlType {
    Juan,
    Hua,
    Fanwai,
    Current,
    Local,
    Upscale,
}