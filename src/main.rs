use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};
use parse_int::parse;
use ultimate64::{drives, Rest};

// Clap 4 colors: https://github.com/clap-rs/clap/issues/3234#issuecomment-1783820412
fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Red.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
}

/// Helper function to extract file extension from `file` to a lowercase string
fn get_extension(file: &std::ffi::OsString) -> String {
    std::path::Path::new(&file)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_lowercase()
}

/// Helper function to determine if file is a disk image using its extension
fn check_if_disk_image(file: &std::ffi::OsString) -> Result<()> {
    let ext = get_extension(file);
    if !["d64", "d71", "d81", "g64", "g71"].contains(&ext.as_str()) {
        return Err(anyhow::anyhow!(
            "File extension must be one of: d64, d71, d81, g64, g71"
        ));
    }
    Ok(())
}

/// A fictional versioning CLI
#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "ultimate64")]
#[command(author = "Mikael Lund aka Wombat")]
#[command(about = "Network Control for Ultimate series", version)]
#[command(color = clap::ColorChoice::Auto)]
#[command(styles=styles())]
struct Cli {
    /// Network address of Ultimate64.
    #[clap(env = "ULTIMATE_HOST")]
    host: String,
    /// Subcommand to run
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum DiskImageCmd {
    /// Mount disk image (unfinished)
    Mount {
        /// Image file
        file: std::ffi::OsString,
        /// Drive number
        #[clap(long, short = 'i', default_value = "8")]
        drive_id: u8,
        /// Mount mode: read only (ro), write only (wo), or unlinked (ul)
        #[clap(long, short = 'm', default_value = "ro")]
        mode: drives::MountMode,
    },
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Show drive information
    Drives,
    /// Load file into memory
    Load {
        /// File to load
        file: std::ffi::OsString,
        /// Load address; otherwise deduce from first two bytes in file
        #[clap(long, short = '@', default_value = None)]
        address: Option<String>,
    },
    /// Play Amiga MOD file
    Modplay {
        /// MOD file
        file: std::ffi::OsString,
    },
    /// Disk image operations
    Image {
        #[clap(subcommand)]
        command: DiskImageCmd,
    },
    /// Pause machine
    Pause,
    /// Read n bytes from memory; hex output
    Peek {
        /// Address to read from, e.g. `4096` or `0x1000`
        address: String,
        /// Number of bytes to read
        #[clap(long, short = 'n', default_value = "1")]
        length: u16,
        /// Write to binary file instead of hexdump
        #[clap(long, short = 'o')]
        outfile: Option<std::ffi::OsString>,
        /// Disassemble instead of hexdump
        #[clap(long = "dasm", short = 'd', action, conflicts_with = "outfile")]
        disassemble: bool,
    },
    /// Write a single byte to memory
    Poke {
        /// Address to write to, e.g. `4096` or `0x1000`
        address: String,
        /// Value to write
        value: u8,
    },
    /// Power off machine
    Poweroff,
    /// Load and run PRG or CRT file
    #[command(arg_required_else_help = true)]
    Run {
        /// PRG or CRT file to load and run
        file: std::ffi::OsString,
    },
    /// Reboot machine
    Reboot,
    /// Reset machine
    Reset,
    /// Resume machine
    Resume,
    /// Play SID file
    Sidplay {
        /// SID file
        file: std::ffi::OsString,
        /// Optional song number
        #[clap(short = 'n')]
        songnr: Option<u8>,
    },
}

/// Disassemble `length` bytes from memory, starting at `address`
/// # Panics
/// Panics if the disassembler fails to disassemble the bytes
fn print_disassembled(bytes: &[u8], address: u16) -> Result<()> {
    let instructions = disasm6502::from_addr_array(bytes, address).unwrap();
    for i in instructions {
        println!("{}", i);
    }
    Ok(())
}

fn do_main() -> Result<()> {
    let args = Cli::parse();
    let ultimate = Rest::new(&args.host);

    match args.command {
        Commands::Drives => {
            let drives = ultimate.drives()?;
            println!("{}", drives);
        }
        Commands::Pause => {
            ultimate.pause()?;
        }
        Commands::Poweroff => {
            ultimate.poweroff()?;
        }
        Commands::Peek {
            address,
            length,
            outfile,
            disassemble,
        } => {
            let _address = parse::<u16>(&address)?;
            let data = ultimate.read_mem(_address, length)?;
            if disassemble {
                print_disassembled(&data, _address)?;
            } else if outfile.is_some() {
                std::fs::write(outfile.unwrap(), &data)?;
            } else {
                println!("{:x?}", data);
            }
        }
        Commands::Poke { address, value } => {
            let _address = parse::<u16>(&address)?;
            ultimate.write_mem(_address, &[value])?;
        }
        Commands::Reboot => {
            ultimate.reboot()?;
        }
        Commands::Reset => {
            ultimate.reset()?;
        }
        Commands::Resume => {
            ultimate.resume()?;
        }
        Commands::Run { file } => {
            let data = std::fs::read(&file)?;
            match get_extension(&file).as_str() {
                "crt" => ultimate.run_crt(&data)?,
                _ => ultimate.run_prg(&data)?,
            }
        }
        Commands::Sidplay { file, songnr } => {
            let data = std::fs::read(file)?;
            ultimate.sid_play(&data, songnr)?;
        }
        Commands::Modplay { file } => {
            let data = std::fs::read(file)?;
            ultimate.mod_play(&data)?;
        }
        Commands::Load { file, address } => {
            let address_int = match address {
                Some(address) => Some(parse::<u16>(&address)?),
                None => None,
            };
            let data = std::fs::read(file)?;
            ultimate.load_data(&data, address_int)?;
        }
        Commands::Image { command } => match command {
            DiskImageCmd::Mount {
                file,
                drive_id,
                mode,
            } => {
                check_if_disk_image(&file)?;
                ultimate.mount_disk_image(&file, drive_id, mode)?;
            }
        },
    }
    Ok(())
}

fn main() {
    if let Err(err) = do_main() {
        eprintln!("Error: {}", &err);
        std::process::exit(1);
    }
}
