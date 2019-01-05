import os, subprocess, shutil, threading, time, sys

print("Checking for cl.exe... ", end='', flush=True)
assert b"Microsoft (R) C/C++ Optimizing Compiler" in subprocess.check_output(["cl.exe", "/?"], stderr=subprocess.STDOUT)
print("ok")

print("Checking that cl.exe is for x64... ", end='', flush=True)
assert b"for x64" in subprocess.check_output(["cl.exe", "/?"], stderr=subprocess.STDOUT)
print("ok")

print("Checking that rustc is present and nightly... ", end='', flush=True)
assert b"-nightly" in subprocess.check_output(["rustc.exe", "--version"], stderr=subprocess.STDOUT)
print("ok")

print("Checking for cygwin... ", end='', flush=True)
assert os.path.exists("C:\\cygwin64\\cygwin.bat")
print("ok")

# Set up path to include cygwin
os.environ["PATH"] += os.pathsep + "C:\\cygwin64\\bin"

if len(sys.argv) == 2 and sys.argv[1] == "deepclean":
    # Completely clean box, including cleaning makefiles from autoconf
    if os.path.exists("bochs_build"):
        shutil.rmtree("bochs_build")
    os.chdir("bochservisor")
    subprocess.check_call(["cargo", "clean"])
    os.chdir("..")
elif len(sys.argv) == 2 and sys.argv[1] == "clean":
    # Clean objects and binaries
    os.chdir("bochs_build")
    subprocess.check_call(["C:\\cygwin64\\bin\\bash.exe", "-c", "make all-clean"])
    os.chdir("..")
    os.chdir("bochservisor")
    subprocess.check_call(["cargo", "clean"])
    os.chdir("..")
else:
    # Build bochservisor
    os.chdir("bochservisor")
    subprocess.check_call(["cargo", "build", "--release"])
    os.chdir("..")

    # Go into bochs build directory
    if not os.path.exists("bochs_build"):
        os.mkdir("bochs_build")
    os.chdir("bochs_build")

    # Set the compiler and linker to MSVC. Without this the ./configure script will
    # potentially use GCC which would result in things like "unsigned long" being
    # reported as 8 bytes instead of the 4 bytes they are on Windows
    os.environ["CC"] = "cl.exe"
    os.environ["CXX"] = "cl.exe"
    os.environ["LD"] = "link.exe"

    # If we have not configured bochs before, or if the configure script is newer
    # than the last configure, reconfigure
    if not os.path.exists("bochs_configured") or os.path.getmtime("bochs_configured") < os.path.getmtime("../bochs_config"):
        # Configure bochs
        subprocess.check_call(["C:\\cygwin64\\bin\\bash.exe", "../bochs_config"])

        # Create a marker indicating that bochs is configured
        with open("bochs_configured", "wb") as fd:
            fd.write(b"WOO")
    else:
        print("Skipping configuration as it's already up to date!")

    # Build bochs
    subprocess.check_call(["C:\\cygwin64\\bin\\bash.exe", "-c", "time make -s -j16"])
    os.chdir("..")
