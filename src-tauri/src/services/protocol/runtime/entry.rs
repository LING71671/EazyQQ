//! Electron joins package.main to the app directory, including on Windows.
use std::path::{Path, PathBuf};

fn same_root(left: &Path, right: &Path) -> bool {
    left.components().next().map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        == right.components().next().map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
}

fn relative(base: &Path, target: &Path) -> Result<String, String> {
    if !same_root(base,target) { return Err("Loader bridge is on the wrong volume".into()); }
    let base:Vec<_>=base.components().collect(); let target:Vec<_>=target.components().collect();
    let common=base.iter().zip(&target).take_while(|(a,b)| a.as_os_str().to_string_lossy().eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())).count();
    let mut path=PathBuf::new();
    for _ in common..base.len() { path.push(".."); }
    for component in &target[common..] { path.push(component.as_os_str()); }
    Ok(path.to_string_lossy().replace('\\',"/"))
}

pub fn package_main(app_dir: &Path, loader: &Path) -> Result<String, String> {
    let app=std::path::absolute(app_dir).map_err(|e|e.to_string())?;
    let loader=std::path::absolute(loader).map_err(|e|e.to_string())?;
    if !loader.is_file() { return Err(format!("Private loader is missing: {}",loader.display())); }
    if same_root(&app,&loader) { return relative(&app,&loader); }
    #[cfg(windows)]
    {
        use sha2::{Digest,Sha256};
        let mut roots=Vec::new();
        if let Some(local)=std::env::var_os("LOCALAPPDATA") { roots.push(PathBuf::from(local).join("EazyQQ/loader-bridges")); }
        roots.push(crate::services::accounts::data_root().join("loader-bridges"));
        let drive=app.components().next().ok_or("QQ path has no drive")?.as_os_str();
        roots.push(PathBuf::from(drive).join("\\EazyQQ_Runtime/loader-bridges"));
        let root=roots.into_iter().find(|root|same_root(&app,root)).ok_or("Cannot locate a same-volume loader bridge")?;
        let id=format!("{:x}",Sha256::digest(loader.parent().unwrap().to_string_lossy().as_bytes()));
        let bridge=root.join(&id[..32]);
        let _lock=crate::services::infra::persistence::FileLock::acquire(&root.join(format!("{}.lock",&id[..32])))?;
        junction(&bridge,loader.parent().unwrap())?;
        return relative(&app,&bridge.join(loader.file_name().unwrap()));
    }
    #[cfg(not(windows))]
    { Err("Cross-volume loaders require Windows directory junctions".into()) }
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) -> Result<(),String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::c_void;
    type Handle=*mut c_void;
    #[link(name="kernel32")]
    unsafe extern "system" {
        fn CreateFileW(name:*const u16,access:u32,share:u32,security:*const c_void,disposition:u32,flags:u32,template:Handle)->Handle;
        fn DeviceIoControl(handle:Handle,code:u32,input:*const c_void,input_len:u32,output:*mut c_void,output_len:u32,returned:*mut u32,overlapped:*mut c_void)->i32;
        fn CloseHandle(handle:Handle)->i32;
    }
    let target=std::fs::canonicalize(target).map_err(|e|e.to_string())?;
    if link.exists() {
        if std::fs::canonicalize(link).map_err(|e|e.to_string())? == target { return Ok(()); }
        return Err(format!("Loader bridge is occupied by another directory: {}",link.display()));
    }
    std::fs::create_dir_all(link.parent().unwrap()).map_err(|e|e.to_string())?;
    std::fs::create_dir(link).map_err(|e|e.to_string())?;
    let result=(|| {
        let path:Vec<u16>=link.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle=unsafe {CreateFileW(path.as_ptr(),0x40000000,7,std::ptr::null(),3,0x02200000,std::ptr::null_mut())};
        if handle as isize == -1 { return Err(std::io::Error::last_os_error().to_string()); }
        let printable=target.to_string_lossy();
        let plain=printable.strip_prefix(r"\\?\").unwrap_or(&printable);
        let substitute:Vec<u16>=format!(r"\??\{}",plain).encode_utf16().collect();
        let print:Vec<u16>=plain.encode_utf16().collect();
        let length=8+(substitute.len()+print.len()+2)*2;
        let mut data=Vec::new();
        data.extend_from_slice(&0xA0000003u32.to_le_bytes()); data.extend_from_slice(&(length as u16).to_le_bytes());data.extend_from_slice(&0u16.to_le_bytes());
        for value in [0u16,(substitute.len()*2) as u16,((substitute.len()+1)*2) as u16,(print.len()*2) as u16] {data.extend_from_slice(&value.to_le_bytes());}
        for value in substitute.into_iter().chain(Some(0)).chain(print).chain(Some(0)) {data.extend_from_slice(&value.to_le_bytes());}
        let mut returned=0;
        let ok=unsafe {DeviceIoControl(handle,0x000900A4,data.as_ptr().cast(),data.len() as u32,std::ptr::null_mut(),0,&mut returned,std::ptr::null_mut())};
        let error=std::io::Error::last_os_error(); unsafe {CloseHandle(handle);}
        if ok == 0 {return Err(error.to_string());} Ok(())
    })();
    if result.is_err() { let _=std::fs::remove_dir(link); }
    result
}

#[cfg(all(test,windows))]
mod tests {
    #[test]
    fn directory_bridge_reads_only_its_expected_private_loader() {
        let root=crate::services::logging::workspace_root().join("junction-entry");
        let target=root.join("private");std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("loadNapCat.js"),b"private module").unwrap();
        let bridge=root.join("bridge");super::junction(&bridge,&target).unwrap();
        assert_eq!(std::fs::read(bridge.join("loadNapCat.js")).unwrap(),b"private module");
        assert!(super::junction(&bridge,&root).is_err());
    }
}
