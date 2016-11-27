extern crate regex;
extern crate xml;

use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

use std::error::Error;

use regex::Regex;
use xml::{Event, Parser};

/// Traverses the Inventory and ContentsXML subfolders and returns
/// the contents of the inventory.xml file.
///
/// # Example
/// let inventory = read_ora_inventory(r"c:\Program Files\Oracle\");
fn read_ora_inventory(inventory_loc: &str) -> Result<String, Box<Error>> {
    let file_path = format!(r"{}./Inventory/ContentsXML/inventory.xml", inventory_loc);

    let mut f = try!(File::open(file_path));
    let mut s = String::new();
    f.read_to_string(&mut s).map(|_| s).map_err(Into::into)
}

/// Parses out the SAVED_WITH tag from the Oracle inventory
fn parse_version(inventory: &str) -> Result<String, Box<Error>> {
    let mut p = Parser::new();
    p.feed_str(inventory);

    // Scan file looking for "SAVED_WITH" tag
    let mut found = false;
    let mut version = String::new();
    for event in p {
        match event.unwrap() {
            Event::ElementStart(tag) => {
                if tag.name.as_str() == "SAVED_WITH" {
                    found = true;
                }
            }
            Event::Characters(verstr) => {
                if found {
                    version.push_str(&verstr);
                    found = false;
                }
            }
            _ => (),
        }
    }
    match version.is_empty() {
        true => Err("Oracle version not found in inventory.")?,
        _ => Ok(version),
    }
}

/// Reformats Oracle version to match .NET Oracle driver version string
fn get_net_match_ver(version: &str) -> Result<String, Box<Error>> {

    // split installed version into components
    let split = Regex::new(r"(\d+).(\d+).(\d+).(\d+).(\d+)").unwrap();

    split.captures(version).map(|cap| {
        let cap1 = cap.at(1).unwrap();
        let cap2 = cap.at(2).unwrap();
        let cap3 = cap.at(3).unwrap();
        let cap4 = cap.at(4).unwrap();
        let cap5 = cap.at(5).unwrap();

        let cap3 = if cap3 == "0" { "" } else { cap3 };
        let cap4 = if cap4 == "0" { "" } else { cap4 };

        format!("2.{}{}.{}{}.{}", cap1, cap2, cap3, cap4, cap5)
    }).ok_or("Unable to parse Oracle version.".into())
}

/// Checks if a path refers to a later version
/// ...
/// Example
///
/// Oracle.DataAccess/2.112.1.0__89b483f429c47342/oracle.dataaccess.dll
/// Here we check the second component shown, treating it as a filename.
fn check_version_ok(verpath: &Path, expected: &str) -> bool {

    // get final component -- better way to this?
    let name = verpath.file_name().unwrap().to_str();
    let version = name.unwrap().split("__").next().unwrap();

    // if version > expected, we'll need to remove it
    version <= expected
}

/// Makes an assembly name that we can pass to gacutil /u
///
/// # Example
///
/// Oracle.DataAccess, Version=2.112.1.0, Culture=neutral, PublicKeyToken=89b483f429c47342, processorArchitecture=AMD64
fn make_assembly_name(gac_type: &str, verpath: &Path) -> Result<String, Box<Error>> {

    // get main assembly name
    let name = verpath.parent().ok_or("No parent directory")?;
    let name = name.file_name().ok_or("parent directory has no file name")?;
    let name = name.to_str().ok_or("parent directory is not Unicode")?;

    // get version and key
    let ver_key = verpath.file_name().ok_or("version path has no filename")?;
    let ver_key = ver_key.to_str().ok_or("version path is not Unicode")?;

    let mut ver_key_parts = ver_key.split("__");
    let ver = ver_key_parts.next().ok_or("missing first version component")?;
    let key = ver_key_parts.next().ok_or("missing second version component")?;

    // build assembly name
    let gac_type = match gac_type {
        "GAC_64" => "AMD64",
        "GAC_32" => "x86",
        "GAC_MSIL" => "MSIL",
        _ => "##Unknown Architecture##",
    };

    Ok(format!(
        "{}, Version={}, Culture=neutral, PublicKeyToken={}, processorArchitecture={}",
        name, ver, key, gac_type
    ))
}


/// Scans gac looking for Oracle assemblies that are greater than the passed version
fn scan_gac(gac: &str, expected: &str) -> Result<Vec<String>, Box<Error>> {
    let p = Path::new(gac);
    let gac_folder = p.file_name().ok_or("gac folder doesn't have file name")?;
    let gac_folder = gac_folder.to_str().ok_or("gac folder isn't Unicode")?;

    // create empty Vec<String> to return list of bad assemblies
    let mut vec: Vec<_> = Vec::new();

    for path in fs::read_dir(gac)? {
        let dirpath = path?.path();

        let dirstr = dirpath.to_str().ok_or("directory isn't Unicode")?;

        // Oracle?
        if dirstr.contains("Oracle") {
            let verpath = fs::read_dir(dirstr)?;
            for ver in verpath {
                let verpath = ver?.path();
                if !check_version_ok(&verpath, expected) {
                    vec.push(make_assembly_name(&gac_folder, &verpath)?)
                }
            }
        }
    }
    Ok(vec)
}


fn main() {
    // open Oracle inventory
    let inventory32 = read_ora_inventory(r"c:/Program Files (x86)/Oracle/");
    let inventory64 = read_ora_inventory(r"c:/Program Files/Oracle/");

    // parse version out of XML
    let version32 = inventory32.and_then(|i| parse_version(&i));
    let version64 = inventory64.and_then(|i| parse_version(&i));

    match (version32.as_ref(), version64.as_ref()) {
        (Err(_), Err(_)) => panic!("No Oracle install found."),
        (Ok(version32), Ok(version64)) if version32 != version64 => {
            panic!("Different Oracle versions installed. \
                    Version {} (32-bit) and version {} (64-bit) found.",
                   version32,
                   version64);
        },
        _ => ()
    }

    let ora_version = version32.or(version64).unwrap();

    // get version to match .NET Oracle driver
    let expected = get_net_match_ver(&ora_version).unwrap();
    println!("");
    println!("Expected .NET version is: {}", expected);
    println!("-----------------------------------");

    // scan gac for later versions
    let kill_assembly_list32 = scan_gac("c:/windows/assembly/GAC_32/", &expected).unwrap();
    let kill_assembly_list64 = scan_gac("c:/windows/assembly/GAC_64/", &expected).unwrap();
    
    match kill_assembly_list32.len()+kill_assembly_list64.len() {
        0 => println!("No incompatible assemblies found to remove."),
        _ => (),
    };

    for assembly in kill_assembly_list32.iter().chain(&kill_assembly_list64) {
        println!("gacutil /u \"{}\"", assembly);
    }
}