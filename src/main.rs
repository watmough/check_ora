extern crate regex;
extern crate xml;

use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use regex::Regex;
use xml::{Event, Parser};
use std::path::Path;

/// Traverses the Inventory and ContentsXML subfolders and returns
/// the contents of the inventory.xml file.
///
/// # Example
/// let inventory = read_ora_inventory("c:\\Program Files\\Oracle\\");
fn read_ora_inventory( inventory_loc: &str ) -> io::Result<String> {
    
    // given the Oracle folder under Program Files get the filename
    let mut file_path = String::from(inventory_loc);
    file_path.push_str(".\\Inventory\\ContentsXML\\inventory.xml");
    
    // slurp xml file and return
    let mut f = try!(File::open(file_path));
    let mut s = String::new();
    match f.read_to_string(&mut s) {
        Ok(_) => Ok(s),
        Err(e) => Err(e),
    }
}

/// Parses out the SAVED_WITH tag from the Oracle inventory
fn parse_version( inventory: &str ) -> io::Result<String> {
    
    // Create a new Parser and feed the inventory in
    let mut p = Parser::new();

    // Feed data to be parsed
    p.feed_str(&inventory);
    
    // Scan file looking for "SAVED_WITH" tag
    let mut found = false;
    let mut version = String::new();
    for event in p {
        match event.unwrap() {
            Event::ElementStart(tag) => { 
                if tag.name.as_str()=="SAVED_WITH" {
                    found = true; 
                }},
            Event::Characters(verstr) => { 
                if found { 
                    version.push_str(&verstr);
                    found = false;
                }},
            _ => ()
        }
    }
    match version.is_empty() {
        true => Err(Error::new(ErrorKind::Other, "Oracle version not found in inventory.")),
        _ => Ok(version),
    }
}

/// Reformats Oracle version to match .NET Oracle driver version string
fn get_net_match_ver( version: &str ) -> io::Result<String> {
    
    // split installed version into components
    let mut expected = String::new();
    let split = Regex::new(r"(\d+).(\d+).(\d+).(\d+).(\d+)").unwrap();
    for cmp in split.captures_iter(&*version) {
        expected.push_str("2.");
        expected.push_str(cmp.at(1).unwrap());
        expected.push_str(cmp.at(2).unwrap());
        expected.push_str(".");
        expected.push_str(if cmp.at(3).unwrap()=="0" { "" } else { cmp.at(3).unwrap() });
        expected.push_str(if cmp.at(4).unwrap()=="0" { "" } else { cmp.at(4).unwrap() });
        expected.push_str(".");
        expected.push_str(cmp.at(5).unwrap());
    }
    
    match expected.is_empty() {
        true => Err(Error::new(ErrorKind::Other, "Unable to parse Oracle version.")),
        _ => Ok(expected),
    }
}

/// Checks if a path refers to a later version
/// ...
/// Example
///
/// Oracle.DataAccess/2.112.1.0__89b483f429c47342/oracle.dataaccess.dll
/// Here we check the second component shown, treating it as a filename.
fn check_ver( verpath: &Path, expected: &str ) -> bool {
    
    // get final component -- better way to this?
    let name = verpath.file_name().unwrap().to_str();
    let version = name.unwrap().split("__").next().unwrap();

    // return true if we are ok.
    // if version > expected, we'll need to remove it
    version<=expected
}

/// Makes an assembly name that we can pass to gacutil /u
///
/// # Example
///
/// Oracle.DataAccess, Version=2.112.1.0, Culture=neutral, PublicKeyToken=89b483f429c47342, processorArchitecture=AMD64
fn make_assembly_name( gac_type: &str, verpath: &Path ) -> io::Result<String> {
    
    // get main assembly name
    let name = verpath.parent().unwrap().file_name().unwrap();
    
    // get version and key
    let ver_key = verpath.file_name().unwrap().to_str();
    let v: Vec<&str> = ver_key.unwrap().split("__").collect();
    let ver = v.get(0).unwrap();
    let key = v.get(1).unwrap();

    // build assembly name
    let mut assembly = String::new();
    assembly.push_str(name.to_str().unwrap());
    assembly.push_str(", Version=");
    assembly.push_str(ver);
    assembly.push_str(", Culture=neutral, PublicKeyToken=");
    assembly.push_str(key);
    assembly.push_str(", processorArchitecture=");
    assembly.push_str(match gac_type { "GAC_64" => "AMD64", "GAC_32" => "x86", "GAC_MSIL" => "MSIL", _ => "##Unknown Arch##",});

    Ok(assembly)
}

/// Scans gac looking for Oracle assemblies that are greater than the passed version
fn scan_gac( gac: &str, expected: &str ) -> io::Result<Vec<String>> {
    
    // get gac folder name
    let gac_folder = Path::new(gac).file_name().unwrap().to_str().unwrap();
    
    // regexp to find Oracle
    let oracle = Regex::new(r"^.*Oracle.*$").unwrap();
    
    // create empty Vec<String> to return list of bad assemblies
    let mut vec: Vec<String> = Vec::new();
    
    // iterate over paths
    for path in try!(fs::read_dir(gac)) {
        let dirpath = path.unwrap().path();
        let dirstr = dirpath.to_str().unwrap();

        // Oracle?
        if oracle.is_match(dirstr) {
            
            let verpath = fs::read_dir(dirstr).unwrap();
            for ver in verpath {
                let verpath = ver.unwrap().path();
                if !check_ver( &verpath, expected ) {
                    vec.push(make_assembly_name(&gac_folder,&verpath).unwrap())
                }
            }
        }
    }
    Ok(vec)
}


fn main() {

    // open Oracle inventory
    let inventory32 = match read_ora_inventory("c:\\Program Files (x86)\\Oracle\\") {
        Ok(data) => data,
        _ => "".to_string(),
    };
    let inventory64 = match read_ora_inventory("c:\\Program Files\\Oracle\\") {
        Ok(data) => data,
        _ => "".to_string(),
    };
    
    // parse version out of XML
    let version32 = match !inventory32.is_empty() { 
        true => parse_version(&*inventory32).unwrap(),
        _ => "".to_string(),
    };
    let version64 = match !inventory64.is_empty() { 
        true => parse_version(&*inventory64).unwrap(),
        _ => "".to_string(),
    };
    
    // check drivers match
    if version32.is_empty() && version64.is_empty() { panic!("No Oracle install found."); };
    if !version32.is_empty() && !version64.is_empty() && version32!=version64 {
        println!("Version {} (32-bit) and version {} (64-bit) found.",version32,version64);
        panic!("Different Oracle versions installed.");
    };
    
    // get single expected version
    let ora_version = if version32.is_empty() { version64 } else { version32 };

    // get version to match .NET Oracle driver
    let expected = get_net_match_ver(&*ora_version).unwrap();
    println!("Expected .NET version is: {}",expected);
    println!("");
    
    // scan gac for later versions
    let kill_assembly_list32 = scan_gac("c:/windows/assembly/GAC_32/",&*expected).unwrap();
    for assembly in kill_assembly_list32 {
        println!("gacutil /u \"{}\"",assembly);
    }
    let kill_assembly_list64 = scan_gac("c:/windows/assembly/GAC_64/",&*expected).unwrap();
    for assembly in kill_assembly_list64 {
        println!("gacutil /u \"{}\"",assembly);
    }
}
