#!/bin/sh
set -e

# MiniPanel OpenWrt 一键安装脚本
# 使用方法: ssh 到 OpenWrt 后执行本脚本

PROG_URL="${PROG_URL:-}"  # 可通过环境变量传入预编译二进制下载地址
BIN_DIR="/usr/bin"
DATA_DIR="/etc/minipanel"
INIT_SCRIPT="/etc/init.d/minipanel"

echo "=== MiniPanel OpenWrt 安装脚本 ==="

# 如果是 aarch64，尝试下载预编译版本（如用户已提供 URL）
if [ -n "$PROG_URL" ]; then
    echo "下载预编译二进制..."
    wget -qO "${BIN_DIR}/minipanel" "$PROG_URL" || curl -fsSL -o "${BIN_DIR}/minipanel" "$PROG_URL"
else
    echo "请将本地编译好的 minipanel 二进制上传到 ${BIN_DIR}/minipanel"
    echo "或使用: PROG_URL=http://your-server/minipanel sh install-openwrt.sh"
    exit 1
fi

chmod +x "${BIN_DIR}/minipanel"

# 创建数据目录
mkdir -p "${DATA_DIR}/logs"

# 写入 procd init 脚本
cat > "${INIT_SCRIPT}" <<'EOF'
#!/bin/sh /etc/rc.common
START=99
STOP=10
USE_PROCD=1
PROG=/usr/bin/minipanel
DATA_DIR=/etc/minipanel

start_service() {
    mkdir -p "${DATA_DIR}/logs"
    procd_open_instance minipanel
    procd_set_param command "${PROG}"
    procd_set_param env MINIPANEL_DATA_DIR="${DATA_DIR}"
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_set_param respawn
    procd_close_instance
}

stop_service() {
    killall -q minipanel
}
EOF

chmod +x "${INIT_SCRIPT}"

# 启用开机自启
"${INIT_SCRIPT}" enable

# 启动服务
"${INIT_SCRIPT}" start

echo ""
echo "安装完成！"
echo "  二进制: ${BIN_DIR}/minipanel"
echo "  数据目录: ${DATA_DIR}"
echo "  默认端口: 8080"
echo "  默认账号: admin / password"
echo ""
echo "管理命令:"
echo "  /etc/init.d/minipanel start|stop|restart"
