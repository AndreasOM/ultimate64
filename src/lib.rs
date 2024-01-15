//!
//! Rust library and command line interface for interfacing with [Ultimate-64 and Ultimate-II](https://ultimate64.com)
//! hardware using the
//! [REST API](https://1541u-documentation.readthedocs.io/en/latest/api/api_calls.html).
//!

use anyhow::{Ok, Result};
use log::debug;

pub mod drives;

/// Check if address + length overflows address space
fn check_address_overflow(address: u16, length: u16) -> Result<()> {
    u16::checked_add(address, length).ok_or_else(|| {
        anyhow::anyhow!(
            "Address {:#x} + length {:#x} overflows address space",
            address,
            length
        )
    })?;
    Ok(())
}

/// Helper funtion to extract load address from first two bytes of data, little endian format
fn extract_load_address(data: &[u8]) -> Result<u16> {
    if data.len() < 2 {
        return Err(anyhow::anyhow!(
            "Data must be two or more bytes to detect load address"
        ));
    }
    let load_address = u16::from_le_bytes([data[0], data[1]]);
    Ok(load_address)
}

/// Communication with Ultimate series using
/// the [REST API](https://1541u-documentation.readthedocs.io/en/latest/api/api_calls.html)
///
/// # Examples
/// ~~~
/// use ultimate64::Rest;
/// let ultimate = Rest::new("192.168.1.10");
/// ultimate.reset();
/// ~~~
#[derive(Debug)]
pub struct Rest {
    /// HTTP client
    client: reqwest::blocking::Client,
    /// Header
    url_pfx: String,
}

impl Rest {
    /// Create new Rest instance
    ///
    /// # Arguments
    /// * `host` - Hostname or IP address of Ultimate-64 of Ultimate-II
    pub fn new(host: &str) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            url_pfx: format!("http://{}/v1", host),
        }
    }

    fn put(&self, path: &str) -> Result<()> {
        let url = format!("{}/{}", self.url_pfx, path);
        self.client.put(url).send()?;
        Ok(())
    }

    /// Get version
    pub fn version(&self) -> Result<String> {
        let url = format!("{}/version", self.url_pfx);
        let response = self.client.get(url).send()?;
        let body = response.text()?;
        Ok(body)
    }

    /// Get drives
    pub fn drives(&self) -> Result<String> {
        let url = format!("{}/drives", self.url_pfx);
        let response = self.client.get(url).send()?;
        let body = response.text()?;
        Ok(body)
    }

    /// Load PRG bytes into memory - do NOT run.
    /// The machine resets, and loads the attached program into memory using DMA.
    pub fn load_prg(&self, prg_data: &[u8]) -> Result<()> {
        let url = format!("{}/runners:load_prg", self.url_pfx);
        self.client.post(url).body(prg_data.to_vec()).send()?;
        Ok(())
    }

    /// Load and run PRG bytes into memory
    ///
    /// The machine resets, and loads the attached program into memory using DMA.
    pub fn run_prg(&self, data: &[u8]) -> Result<()> {
        let url = format!("{}/runners:run_prg", self.url_pfx);
        self.client.post(url).body(data.to_vec()).send()?;
        Ok(())
    }

    /// Start supplied cartridge file
    ///
    /// The ‘crt’ file is attached to the POST request.
    /// The machine resets, with the attached cartridge active.
    /// It does not alter the configuration of the Ultimate.
    pub fn run_crt(&self, data: &[u8]) -> Result<()> {
        debug!("Run CRT file of {} bytes", data.len());
        let url = format!("{}/runners:run_crt", self.url_pfx);
        self.client.post(url).body(data.to_vec()).send()?;
        Ok(())
    }

    /// Reset machine
    pub fn reset(&self) -> Result<()> {
        debug!("Reset machine");
        self.put("machine:reset")?;
        Ok(())
    }
    /// Reboot machine
    pub fn reboot(&self) -> Result<()> {
        debug!("Reboot machine");
        self.put("machine:reboot")?;
        Ok(())
    }
    /// Pause machine
    pub fn pause(&self) -> Result<()> {
        debug!("Pause machine");
        self.put("machine:pause")?;
        Ok(())
    }
    /// Resume machine
    pub fn resume(&self) -> Result<()> {
        debug!("Resume machine");
        self.put("machine:resume")?;
        Ok(())
    }
    /// Poweroff machine
    pub fn poweroff(&self) -> Result<()> {
        debug!("Poweroff machine");
        self.put("machine:poweroff")?;
        Ok(())
    }

    /// Write data to memory using a POST request
    pub fn write_mem(&self, address: u16, data: &[u8]) -> Result<()> {
        debug!("Write {} bytes to 0x{:#x}", data.len(), address);
        check_address_overflow(address, data.len() as u16)?;
        let url = format!("{}/machine:writemem?address={:x}", self.url_pfx, address);
        self.client.post(url).body(data.to_vec()).send()?;
        Ok(())
    }

    /// Read `length` bytes from `address`
    pub fn read_mem(&self, address: u16, length: u16) -> Result<Vec<u8>> {
        debug!("Read {} bytes from 0x{:#x}", length, address);
        check_address_overflow(address, length)?;
        let url = format!(
            "{}/machine:readmem?address={:x}&length={}",
            self.url_pfx, address, length
        );
        let response = self.client.get(url).send()?;
        let bytes = response.bytes()?.to_vec();
        Ok(bytes)
    }

    /// Play SID file
    pub fn sid_play(&self, siddata: &[u8], songnr: Option<u8>) -> Result<()> {
        let url = match songnr {
            Some(songnr) => format!("{}/runners:sidplay?songnr={}", self.url_pfx, songnr),
            None => format!("{}/runners:sidplay", self.url_pfx),
        };
        self.client.post(url).body(siddata.to_vec()).send()?;
        Ok(())
    }

    /// Play amiga MOD file
    pub fn mod_play(&self, moddata: &[u8]) -> Result<()> {
        let url = format!("{}/runners:modplay", self.url_pfx);
        self.client.post(url).body(moddata.to_vec()).send()?;
        Ok(())
    }

    /// Load data into memory using either a custom address, or deduce the
    /// load address from the first two bytes of the data (little endian).
    /// In the case of the latter, the first two bytes are not written to memory.
    pub fn load_data(&self, data: &[u8], address: Option<u16>) -> Result<()> {
        match address {
            Some(address) => self.write_mem(address, data),
            None => {
                let load_address = extract_load_address(data)?;
                debug!("Detected load address: 0x{:#x}", load_address);
                self.write_mem(load_address, &data[2..]) // skip first two bytes
            }
        }
    }

    /// Mount disk image
    pub fn mount_disk_image(
        &self,
        path: &std::ffi::OsStr,
        drive_id: u8,
        mount_mode: drives::MountMode,
    ) -> Result<()> {
        let url = format!(
            "{}/v1/drives/{}:mount?mode={}",
            self.url_pfx,
            drive_id,
            String::from(mount_mode)
        );
        let file = std::fs::File::open(path)?;
        self.client.post(url).body(file).send()?;
        Err(anyhow::anyhow!(
            "Disk mounting is unfinished and currently doesn't work"
        ))
    }
}
