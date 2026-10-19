//! CLI argument parsing for AnimationCraft desktop app.

pub struct Args {
    pub help: bool,
    pub version: bool,
    pub control_port: Option<u16>,
    pub image_path: Option<String>,
}

pub const USAGE: &str = "\
animationcraft: 2D/3D turnaround animation studio

USAGE
  animationcraft [OPTIONS] [IMAGE]

OPTIONS
  --control <PORT>  Start localhost JSON-lines control server on 127.0.0.1:<PORT>
  --help            Print this help text
  --version         Print the version
  IMAGE             Optional image to validate on startup (runs intake.validate)
";

impl Args {
    pub fn parse(args: std::env::Args) -> Self {
        let mut help = false;
        let mut version = false;
        let mut control_port = None;
        let mut image_path = None;

        let collected: Vec<String> = args.skip(1).collect();
        let mut i = 0;
        while i < collected.len() {
            match collected[i].as_str() {
                "--help" | "-h" => help = true,
                "--version" | "-V" => version = true,
                "--control" | "--bridge" => {
                    if let Some(p) = collected.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                        control_port = Some(p);
                        i += 1;
                    }
                }
                other if !other.starts_with('-') => {
                    image_path = Some(other.to_string());
                }
                _ => {}
            }
            i += 1;
        }

        Self {
            help,
            version,
            control_port,
            image_path,
        }
    }
}
