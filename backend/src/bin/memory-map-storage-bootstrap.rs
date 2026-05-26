use {
	anyhow::Context,
	backend::storage::{
		StorageClient,
		StorageConfig,
	},
	std::{
		io::{
			Read,
			Write,
		},
		net::{
			TcpStream,
			ToSocketAddrs,
		},
		time::Duration,
	},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = StorageConfig::from_env().context("Failed to read storage bootstrap config")?;
	ensure_rustfs_health(&config.endpoint_url)
		.context("Failed to verify RustFS health endpoint")?;
	let storage =
		StorageClient::from_storage_config(&config).context("Failed to build storage client")?;
	storage.ensure_bucket_ready().await.context("Failed to ensure S3 bucket readiness")?;
	Ok(())
}

fn ensure_rustfs_health(endpoint_url: &str) -> anyhow::Result<()> {
	let endpoint = parse_http_endpoint(endpoint_url)?;
	let timeout = Duration::from_secs(5);
	let mut stream = connect_with_timeout(&endpoint.host, endpoint.port, timeout)?;
	stream.set_read_timeout(Some(timeout))?;
	stream.set_write_timeout(Some(timeout))?;
	write!(
		stream,
		"GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
		endpoint.health_path, endpoint.authority
	)?;

	let mut response = String::new();
	stream.read_to_string(&mut response)?;
	let (headers, body) = response
		.split_once("\r\n\r\n")
		.context("RustFS health response did not contain HTTP headers")?;
	let status_code = headers
		.lines()
		.next()
		.and_then(|status| status.split_whitespace().nth(1))
		.context("RustFS health response did not contain an HTTP status")?;
	if status_code != "200" {
		anyhow::bail!("RustFS health endpoint returned HTTP {status_code}");
	}
	if !(body.contains("\"service\"") && body.contains("\"rustfs-endpoint\"")) {
		anyhow::bail!("RustFS health endpoint did not identify a RustFS service");
	}
	Ok(())
}

#[derive(Debug)]
struct HttpEndpoint {
	authority: String,
	host: String,
	port: u16,
	health_path: String,
}

fn parse_http_endpoint(endpoint_url: &str) -> anyhow::Result<HttpEndpoint> {
	let endpoint_url = endpoint_url.trim().trim_end_matches('/');
	let rest = endpoint_url
		.strip_prefix("http://")
		.context("RustFS bootstrap currently supports only http:// S3 endpoints")?;
	let (authority, base_path) = rest.split_once('/').unwrap_or((rest, ""));
	if authority.is_empty() {
		anyhow::bail!("S3 endpoint URL is missing a host");
	}
	let (host, port) = parse_authority(authority)?;
	let base_path = base_path.trim_matches('/');
	let health_path = if base_path.is_empty() {
		"/health/ready".to_string()
	} else {
		format!("/{base_path}/health/ready")
	};

	Ok(HttpEndpoint {
		authority: authority.to_string(),
		host,
		port,
		health_path,
	})
}

fn parse_authority(authority: &str) -> anyhow::Result<(String, u16)> {
	if let Some(rest) = authority.strip_prefix('[') {
		let (host, rest) =
			rest.split_once(']').context("Bracketed IPv6 endpoint is missing a closing bracket")?;
		let port = rest.strip_prefix(':').map(parse_port).transpose()?.unwrap_or(80);
		return Ok((host.to_string(), port));
	}

	let (host, port) = if let Some((host, port)) = authority.split_once(':') {
		(host.to_string(), parse_port(port)?)
	} else {
		(authority.to_string(), 80)
	};
	if host.is_empty() {
		anyhow::bail!("S3 endpoint URL is missing a host");
	}
	Ok((host, port))
}

fn parse_port(port: &str) -> anyhow::Result<u16> {
	port.parse().context("S3 endpoint URL port must be a valid TCP port")
}

fn connect_with_timeout(
	host: &str,
	port: u16,
	timeout: Duration,
) -> anyhow::Result<TcpStream> {
	let mut last_error = None;
	for address in (host, port).to_socket_addrs()? {
		match TcpStream::connect_timeout(&address, timeout) {
			Ok(stream) => return Ok(stream),
			Err(error) => last_error = Some(error),
		}
	}

	if let Some(error) = last_error {
		return Err(error).with_context(|| format!("Failed to connect to {host}:{port}"));
	}
	anyhow::bail!("S3 endpoint URL resolved no addresses for {host}:{port}");
}
