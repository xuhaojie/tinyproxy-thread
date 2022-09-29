use {log::*, url::Url, anyhow::{*, Result},  dotenv};
use std::{thread, io::{Read, Write}, net::{SocketAddr, TcpListener, TcpStream}};

const BUFFER_SIZE: usize = 256;

fn main() -> Result<()> {
	env_logger::init();
	dotenv::dotenv().ok();
	let server_address = dotenv::var("PROXY_ADDRESS").unwrap_or("0.0.0.0:8088".to_owned());
	
	let server = TcpListener::bind(&server_address).unwrap();
	info!("listening on {}", &server_address);
	//let mut thread_pool = Threadpool::new(THREADS);
	while let Result::Ok((client_stream, client_addr)) = server.accept() {
		thread::spawn(move || {
			match process_client(client_stream, client_addr) { anyhow::Result::Ok(()) => (), Err(e) => error!("error: {}", e), }
		});
	}

	Ok(())
}

fn process_client(mut client_stream: TcpStream, client_addr: SocketAddr) -> Result<()> {
	let mut buf : [u8; BUFFER_SIZE] = unsafe { std::mem::MaybeUninit::uninit().assume_init() }; 
	let count = client_stream.read(&mut buf)?;
	if count == 0 { return Ok(()); }

	let request = String::from_utf8_lossy(&buf);
	let mut lines = request.lines();
	let line = match lines.next() {Some(l) => l, None => return Err(anyhow!("bad request")) };
	let mut fields = line.split_whitespace();
	let method = match fields.next() {Some(m) => m, None => return Err(anyhow!("can't find request method"))};
	let url_str = match fields.next() {Some(u) =>  u, None => return Err(anyhow!("can't find url"))};

	let (https, address) = match method {
		"CONNECT"  => (true, String::from(url_str)),
		_ => {
			let url =  Url::parse(url_str)?;
			match url.host() {
				Some(addr) => (false, format!("{}:{}", addr.to_string(), url.port().unwrap_or(80))),
				_ => return Err(anyhow!("can't find host from url")),
			}
		}
	};

	info!("{} -> {}", client_addr.to_string(), line);

	let mut server_stream = TcpStream::connect(address)?;

	if https { client_stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;} 
	else { server_stream.write_all(&buf[..count])?; }

	let mut server_stream_clone =  server_stream.try_clone().unwrap();
	let mut client_stream_clone = client_stream.try_clone().unwrap();
	let copy_task_rx = thread::spawn(move || -> Result<()> {
		std::io::copy(&mut server_stream_clone, &mut client_stream_clone)?;
		Ok(())
	});

	std::io::copy(&mut client_stream,  &mut server_stream)?;

	copy_task_rx.join().unwrap()?;

	Ok(())
}