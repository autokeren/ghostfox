version=152.0.4
release=beta.30
closedsrc_rev=1.0.0
# NOTE: do NOT export BUILD_TARGET/arch here. This file is `include`d by the
# Makefile as MAKE syntax (not sourced by bash), and make-exported variables
# override the environment for every recipe — which clobbered multibuild's
# per-matrix target and broke all cross-compile builds. For a local default
# target, export BUILD_TARGET in your shell or pass it to make explicitly.
