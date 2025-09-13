use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tracing::{debug, error, info, warn};
use tracing_subscriber;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "vlc-control")]
#[command(about = "A VLC remote control server")]
struct Args {
    /// logging level
    #[arg(short, long, value_enum, default_value_t = LogLevel::Info)]
    log_level: LogLevel,
    
    /// VLC server address
    #[arg(long, default_value = "127.0.0.1:54322")]
    vlc_address: String,
    
    /// TCP listening address
    #[arg(long, default_value = "0.0.0.0:55550")]
    tcp_address: String,
    
    /// UDP listening address
    #[arg(long, default_value = "0.0.0.0:55551")]
    udp_address: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum LogLevel {
    /// Only errors
    Error,
    /// Warnings and errors
    Warn,
    /// Info, warnings, and errors (default)
    Info,
    /// Debug and above (verbose)
    Debug,
    /// All log messages (very verbose)
    Trace,
}

impl LogLevel {
    fn as_filter_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

const MAX_COMMAND_SIZE: usize = 128;
const ALLOWED_COMMANDS: &[&str] = &[
    "play", "pause", "stop", "next", "prev", "playlist", "frame", 
    "pi_restart_vlc", "pi_shutdown", "pi_reboot"
];

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize structured logging with CLI argument or environment variable
    let filter = if std::env::var("RUST_LOG").is_ok() {
        // If RUST_LOG is set, use it (environment variable takes precedence)
        EnvFilter::from_default_env()
    } else {
        // Otherwise, use the CLI argument
        EnvFilter::new(format!("vlc_control={}", args.log_level.as_filter_str()))
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    info!(
        vlc_addr = %args.vlc_address,
        tcp_addr = %args.tcp_address, 
        udp_addr = %args.udp_address,
        "Starting VLC Controller servers..."
    );
    
    // Clone addresses for the async tasks
    let tcp_addr = args.tcp_address.clone();
    let udp_addr = args.udp_address.clone();
    let vlc_addr = args.vlc_address.clone();
    
    tokio::select! {
        res = run_tcp_server(&tcp_addr, &vlc_addr) => {
            if let Err(e) = res {
                error!(error = %e, "TCP server crashed");
            }
        },
        res = run_udp_server(&udp_addr, &vlc_addr) => {
            if let Err(e) = res {
                error!(error = %e, "UDP server crashed");
            }
        },
    }
    Ok(())
}

/// TCP listener
async fn run_tcp_server(tcp_addr: &str, vlc_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(tcp_addr).await?;
    info!(address = tcp_addr, "TCP Server listening");
    
    let vlc_addr = vlc_addr.to_string(); // Clone for use in spawned tasks

    loop {
        // Accept a new connection.
        let (socket, addr) = listener.accept().await?;
        info!(client_addr = %addr, "Got inbound TCP connection");

        // Spawn a new asynchronous task
        let vlc_addr_clone = vlc_addr.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_tcp_connection(socket, &vlc_addr_clone).await {
                error!(client_addr = %addr, error = %e, "Error handling TCP client");
            }
        });
    }
}

/// Handles a TCP client connection 
async fn handle_tcp_connection(mut socket: TcpStream, vlc_addr: &str) -> Result<()> {
    // Split the socket into separate reader and writer halves.
    let (reader, _writer) = socket.split();

    // BufReader now takes ownership of the `reader` half only.
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    // Read lines from the client in a loop.
    while buf_reader.read_line(&mut line).await? != 0 {
        let command = line.trim();
        debug!(command = %command, "Received TCP message");
        process_command(line.as_bytes(), vlc_addr).await?;

        line.clear(); // Clear the buffer for the next line.
    }
    
    info!("TCP client disconnected cleanly");
    Ok(())
}

/// UDP listener
async fn run_udp_server(udp_addr: &str, vlc_addr: &str) -> Result<()> {
    let socket = UdpSocket::bind(udp_addr).await?;
    info!(address = udp_addr, "UDP Server listening");
    let mut buf = [0; 1024];

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let command = String::from_utf8_lossy(&buf[..len]);
        debug!(client_addr = %addr, command = %command.trim(), "Got UDP datagram");
        process_command(&buf[..len], vlc_addr).await?;
    }
}

/// Command dispatcher
async fn process_command(data: &[u8], vlc_addr: &str) -> Result<()> {
    // Size validation
    if data.len() > MAX_COMMAND_SIZE {
        anyhow::bail!("Command too large: {} bytes (max {})", data.len(), MAX_COMMAND_SIZE);
    }
    // convert byte slice to string
    let command = std::str::from_utf8(data)?.trim();
    // Validate the command
    if command.starts_with("pi_") && !ALLOWED_COMMANDS.contains(&command) {
        warn!(command = %command, "Blocked unauthorized system command");
        anyhow::bail!("Unauthorized system command: {}", command);
    }

    match command {
        "pi_restart_vlc" => {
            info!("Executing VLC restart command");
            let status = Command::new("systemctl")
                .args(["--user", "restart", "vlc-loader.service"])
                .status()?; // .status() waits for the command to finish.
            if status.success() {
                info!("VLC restart command completed successfully");
            } else {
                warn!(exit_code = status.code(), "VLC restart command failed");
            }
        }
        "pi_shutdown" => {
            warn!("Executing system shutdown command");
            let status = Command::new("sudo").args(["shutdown", "-h", "now"]).status()?;
            if status.success() {
                info!("Shutdown command completed successfully");
            } else {
                error!(exit_code = status.code(), "Shutdown command failed");
            }
        }
        "pi_reboot" => {
            warn!("Executing system reboot command");
            let status = Command::new("sudo").args(["shutdown", "-r", "now"]).status()?;
            if status.success() {
                info!("Reboot command completed successfully");
            } else {
                error!(exit_code = status.code(), "Reboot command failed");
            }
        }
        _ => {
            // Assume it's a command for VLC.
            debug!(command = %command, "Forwarding command to VLC");
            forward_to_vlc_with_retry(data, vlc_addr).await?;
        }
    }
    Ok(())
}

// 3 attempts to connect to vlc then error
async fn forward_to_vlc_with_retry(command: &[u8], vlc_addr: &str) -> Result<()> {
    let max_retries = 3;
    let mut retry_delay = Duration::from_millis(100);
    
    for attempt in 1..=max_retries {
        match forward_to_vlc(command, vlc_addr).await {
            Ok(response) => return Ok(response),
            Err(e) if attempt < max_retries => {
                warn!(
                    attempt = attempt,
                    error = %e,
                    delay_ms = retry_delay.as_millis(),
                    "VLC connection failed, retrying..."
                );
                tokio::time::sleep(retry_delay).await;
                retry_delay *= 2;
            }
            Err(e) => {
                error!(attempts = max_retries, error = %e, "VLC connection failed permanently");
                return Err(e);
            }
        }
    }
    unreachable!()
}

/// Connects to VLC to forward a command.
async fn forward_to_vlc(command: &[u8], vlc_addr: &str) -> Result<()> {
    // Make the stream mutable so the reader can borrow it.
    let mut stream = TcpStream::connect(vlc_addr).await?;
    debug!(address = vlc_addr, "Connected to VLC");

    // The BufReader takes a *mutable* borrow of the stream.
    let mut reader = BufReader::new(&mut stream);
    let mut response_buf = Vec::new();

    // Read the initial prompt
    reader.read_until(b'>', &mut response_buf).await?;
    debug!("Read VLC initial prompt");
    
    // To write, get a mutable reference to the underlying
    // stream directly from the reader itself.
    reader.get_mut().write_all(command).await?;
    debug!(command = %String::from_utf8_lossy(command).trim(), "Sent command to VLC");

    // Clear the buffer and continue using the same reader for the reply.
    response_buf.clear();
    reader.read_until(b'>', &mut response_buf).await?;

    let response = String::from_utf8_lossy(&response_buf);
    debug!(response = %response.trim(), "VLC response received\n");

    Ok(())
}
