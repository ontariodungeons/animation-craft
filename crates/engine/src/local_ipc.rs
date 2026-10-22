//! Same-user local control endpoint validation. No network listener or access expansion.

#[cfg(unix)]
pub fn private_path(path:&std::path::Path,directory:bool)->Result<(),String>{
 use std::os::unix::fs::{MetadataExt,FileTypeExt};
 let m=std::fs::symlink_metadata(path).map_err(|e|format!("{}: {e}",path.display()))?;
 #[cfg(target_os="linux")]
 let uid=rustix::process::geteuid().as_raw();
 #[cfg(not(target_os="linux"))]
 return Err("Local endpoint ownership check is not implemented on this platform".into());
 #[cfg(target_os="linux")]
 if m.file_type().is_symlink()||m.uid()!=uid||m.mode()&0o077!=0||if directory{!m.is_dir()}else{!m.file_type().is_socket()}{return Err(format!("Refusing unowned, non-private, symlinked or wrong-type local endpoint {}",path.display()));}
 Ok(())
}
#[cfg(unix)]
pub fn peer(stream:&std::os::unix::net::UnixStream)->Result<(),String>{
 #[cfg(target_os="linux")]{
 let credentials=rustix::net::sockopt::socket_peercred(stream).map_err(|e|e.to_string())?;
 if credentials.uid!=rustix::process::geteuid(){return Err("Refusing local peer: same-user credentials not verified".into());}Ok(())
 }
 #[cfg(not(target_os="linux"))] {Err("Local peer credentials not implemented on this platform".into())}
}
