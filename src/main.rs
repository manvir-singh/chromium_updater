extern crate libc;
extern crate hyper;
extern crate hyper_native_tls;

use std::str;
use std::ptr;
use std::thread;
use std::io::Read;
use std::fs::{File, remove_file};
use std::io::prelude::*;
use std::process::Command;
use std::ffi::{CString, CStr};

use libc::{c_char, c_ulong, c_void};

use hyper::Client;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;
use hyper::header::Connection;

#[link(name = "version")]
extern "stdcall" {
	pub fn GetFileVersionInfoSizeA(path: *const c_char, ignore: *const u32) -> c_ulong;
	pub fn GetFileVersionInfoA(path: *const c_char, ignore: *const u32, buff_size: *const c_ulong, info_size_buff: *mut c_void) -> c_ulong;
	pub fn VerQueryValueA(info_buff: *const c_void, info_block: *const c_char, result: &mut *mut c_void, ignore: *const u32) -> c_ulong;
}

fn get_file_version_info_size(path: &str) -> u32 {
	unsafe { GetFileVersionInfoSizeA(CString::new(path).unwrap().as_ptr(), ptr::null()) }
}

fn get_file_version_info(exe_path: &str, size: &u32) -> Option<Vec<u8>> {
	let exe_path = CString::new(exe_path).unwrap();
	let mut buff = vec![0; *size as usize]; // TODO Check this
    let result = unsafe { GetFileVersionInfoA(exe_path.as_ptr(), ptr::null(), size as *const c_ulong, buff.as_mut_ptr() as *mut c_void) };
    if result != 1 { return None }
    Some(buff)
}

fn ver_query_value(version_info_buff: &Vec<u8>) -> &'static str {
    let mut result_ptr = ptr::null_mut();
    let result = unsafe {
    	VerQueryValueA(version_info_buff.as_ptr() as *const c_void, CString::new("\\StringFileInfo\\040904B0\\LastChange").unwrap().as_ptr(), &mut result_ptr, ptr::null())
    };
    if result != 1 {
        panic!("VerQueryValueA failed!");
    }
    unsafe { CStr::from_ptr(result_ptr as *const c_char).to_str().unwrap() }
}

/*
* returns 0 if unable to get local verion
*/
fn get_local_version(exe_path: &str) -> String {
	let size = get_file_version_info_size(exe_path);
	let buff = match get_file_version_info(exe_path, &size) {
		Some(val) => val,
		None => return "0".to_string(),
	};

	let last_change = ver_query_value(&buff).split('#').last().unwrap();
	String::from(last_change.trim_matches('}'))
}

fn get_remote_version(url: &str) -> String {
	let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);
    let mut response = client.get(url).header(Connection::close()).send().unwrap();
    let mut body = String::new();
    response.read_to_string(&mut body).unwrap();
    body
}

fn dl_intaller(out_path: &str, last_change: &str) {
	let installer_url = "https://www.googleapis.com/download/storage/v1/b/chromium-browser-snapshots/o/Win_x64%2F".to_string() + last_change + "%2Fmini_installer.exe?alt=media";
	let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);
    let mut response = client.get(&installer_url).header(Connection::close()).send().unwrap();
    let mut body = Vec::new();
    response.read_to_end(&mut body).unwrap();

    let mut out_file = File::create(out_path).unwrap();
	out_file.write_all(&body).expect("Failed to write installer to disk!");
}

fn run_installer(path: &str) {
	Command::new(path).output().expect("Failed to run installer!");
	remove_file(path).expect("Failed to delete installer!");
}

fn update(out_path: &str, remote: &str) {
	dl_intaller(out_path, remote);
	run_installer(out_path);
}

fn main() {
	const LAST_CHANGE_URL: &str = "https://www.googleapis.com/download/storage/v1/b/chromium-browser-snapshots/o/Win_x64%2FLAST_CHANGE?alt=media";
	const OUT_PATH: &str = "mini_installer.exe";
	let chrome_exe = env!("LOCALAPPDATA").to_string() + "\\Chromium\\Application\\chrome.exe";

	let remote_thread = thread::spawn(move|| {
		get_remote_version(LAST_CHANGE_URL)
	});

	let local_thread = thread::spawn(move|| {
		get_local_version(&chrome_exe)
	});

	let remote = remote_thread.join().unwrap();
	let local = local_thread.join().unwrap();
	
	println!("Local: {}", local);
	println!("Remote: {}", remote);

	if local != remote {
		update(OUT_PATH, &remote);
	}
}
