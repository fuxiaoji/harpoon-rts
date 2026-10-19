零构建界面：无需 npm 或打包，直接 `source ~/.cargo/env && cargo run -p harpoon-server -- --port 8799 --scenarios scenarios --web web`，然后浏览器打开 <http://127.0.0.1:8799/> 即可（页面通过 WebSocket `/ws?side=…` 接收态势，连接中断时自动切换为每 500ms 轮询 `/api/state`）。

战局复盘入口为指挥台的“战局复盘”或 `replay.html`，支持蓝方、红方及上帝视角、时间轴和档案导入导出。部署本地教学对局与采样精度说明见 [battle-replay.md](../docs/battle-replay.md)。复盘页只读静态文件，不更改进行中的战局。
