version=152.0.4
release=beta.30
closedsrc_rev=1.0.0
# Local-build convenience defaults ONLY: never override an explicitly set
# BUILD_TARGET (multibuild.py exports it per matrix job; hardcoding it here
# broke every cross-compile on CI).
export BUILD_TARGET="${BUILD_TARGET:-linux,x86_64}"
export arch="${arch:-x86_64}"
