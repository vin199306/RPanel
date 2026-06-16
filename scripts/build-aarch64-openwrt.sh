#!/bin/bash
set -e

# =============================================================================
# MiniPanel aarch64 OpenWrt 交叉编译脚本
# =============================================================================
# 前置条件：
#   1. 安装 Rust (rustup)
#   2. 添加目标平台: rustup target add aarch64-unknown-linux-musl
#   3. 下载 OpenWrt SDK 或工具链，并确保交叉编译器在 PATH 中
#
# OpenWrt SDK 下载示例 (请根据你的 OpenWrt 版本和路由器型号替换 URL)：
#   wget https://downloads.openwrt.org/releases/23.05.3/targets/rockchip/armv8/openwrt-sdk-23.05.3-rockchip-armv8_gcc-12.3.0_musl.Linux-x86_64.tar.xz
#   tar -xJf openwrt-sdk-*.tar.xz
#   export PATH=$PWD/openwrt-sdk-*/staging_dir/toolchain-aarch64_*/bin:$PATH
# =============================================================================

TARGET="aarch64-unknown-linux-musl"
TOOLCHAIN_PREFIX="aarch64-openwrt-linux-musl"

# 检查交叉编译器是否存在
if ! command -v ${TOOLCHAIN_PREFIX}-gcc &> /dev/null; then
    echo "错误: 找不到交叉编译器 ${TOOLCHAIN_PREFIX}-gcc"
    echo "请将 OpenWrt 工具链的 bin 目录添加到 PATH 环境变量中。"
    exit 1
fi

# 设置 Cargo 使用的交叉编译工具
export CC_aarch64_unknown_linux_musl=${TOOLCHAIN_PREFIX}-gcc
export CXX_aarch64_unknown_linux_musl=${TOOLCHAIN_PREFIX}-g++
export AR_aarch64_unknown_linux_musl=${TOOLCHAIN_PREFIX}-ar
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=${TOOLCHAIN_PREFIX}-gcc

echo "开始构建 MiniPanel for ${TARGET} ..."
cargo build --release --target ${TARGET}

echo ""
echo "构建完成:"
echo "  二进制文件: target/${TARGET}/release/minipanel"
echo ""

# 检查是否为静态链接
file "target/${TARGET}/release/minipanel"

# 统计体积
ls -lh "target/${TARGET}/release/minipanel"
