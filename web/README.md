零构建界面：无需 npm 或打包，直接 `source ~/.cargo/env && cargo run -p harpoon-server -- --port 8799 --scenarios scenarios --web web`，然后浏览器打开 <http://127.0.0.1:8799/> 进入大厅。战术指挥台保留于`command.html`，通过WebSocket `/ws?side=…` 接收态势，连接中断时自动切换为每500ms轮询`/api/state`；大厅本身不会发送模拟命令。

战局复盘入口为指挥台的“战局复盘”或 `replay.html`，支持蓝方、红方及上帝视角、时间轴和档案导入导出。部署本地教学对局与采样精度说明见 [battle-replay.md](../docs/battle-replay.md)。复盘页只读静态文件，不更改进行中的战局。

根入口`index.html`现为统一大厅，原指挥台为`command.html`。大厅提供真实当前局、任务/训练预览与确认启动、复盘目录和偏好设置；详情见[command-lobby.md](../docs/command-lobby.md)。修改想定后运行`python scripts/build-lobby-catalog.py`更新只含公共初设摘要的元数据。
