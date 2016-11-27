# check_ora
Generate gacutil.exe cleanup code for incorrect Oracle assemblies in Windows GAC

My day job is supporting a piece of software that just started using the DevArt 
Oracle drivers, which utilize the Oracle.DataAccess .NET drivers.

However, the assemblies in the .NET GAC may not uninstall correctly, and I have
encountered later assemblies that do not match the installed Oracle client, and 
which therefore do not function.

The painful resolution is to send the user a copy of gacutil.exe and have them 
export a listing of the GAC to identify driver mismatches.

This Rust code automates identifying Oracle assembly mismatches in the GAC.
