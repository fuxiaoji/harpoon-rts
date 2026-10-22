# 卫星底图与陆地轮廓：官方来源核查

核查日期2026-10-21。用途是游戏地图显示、公开海岸与陆地判定；不收集现实军事设施内部布局、实际兵力或攻击参数。新`taiwan_island_exercise`已接入GSHHG2.3.7派生岸线，旧想定和历史存档冻结岸线不变。

## CMO的对应能力

[CMO官方手册§5.1](https://command.matrixgames.com/manual/05-1-simulation-area/)将卫星影像、地形/陆地覆盖及叠加层分别说明。[官方地图介绍](https://command.matrixgames.com/?p=4954)列出Blue Marble与Sentinel-2等图层。不能据此把CMO瓦片当作可直接复制的公开资产；[官方影像变更公告](https://www.matrixgames.com/news/command-modern-operations-satellite-imagery-removal)记录过第三方影像移除。

## 数据分账

| 数据 | 可用内容与版本 | 授权/接入边界 |
|---|---|---|
| [NASA GIBS](https://nasa-gibs.github.io/gibs-api-docs/) | 影像可视化瓦片；现有项目使用Blue Marble 2004-08 | 保留NASA署名；显示层，不包含可直接用于碰撞的陆地多边形 |
| [Natural Earth](https://www.naturalearthdata.com/about/terms-of-use/) | 项目已冻结5.1.1陆地多边形，1:1000万 | 公共领域；“10m”是比例尺代号，不是10米分辨率；不能表现港内细节 |
| [GSHHG/NOAA说明](https://www.ngdc.noaa.gov/mgg/shorelines/shorelines.html) | 2.3.7；WGS84、5档分辨率、分层闭合岸线/湖泊多边形 | LGPL，须保留原许可；旧数据，不承诺当代大型比例尺/港内精度 |
| [GSHHG官方维护仓库](https://github.com/GenericMappingTools/gshhg-gmt/blob/master/README.md) | GMT版本的源码、说明和发布入口 | GMT包是为GMT编码的数据，不把它误读为普通GeoJSON；需转换及拓扑验收 |
| [EOxCloudless](https://cloudless.eox.at/license-non-commercial) | 全球Sentinel-2无云年度影像；新热开场已接入2016图层`s2cloudless` | 2016为CC BY4.0，按官方要求显示署名；2018–2025为CC BY-NC-SA4.0，本轮未启用这些限制不同的图层 |

NOAA旧`.../data/gshhg/latest/`和其readme链接在本次打开时404，不能把旧链接列成已成功下载。实际从[SOEST官方二进制包](https://ftp.soest.hawaii.edu/gshhg/gshhg-bin-2.3.7.zip)下载118,617,033字节，ZIP全部成员CRC验证通过；SHA256为`28600e8f7a08645aab43079326df6504212ec5ccb2b4bcf3b5f4f12ed60e82bc`。Python初次TLS验证失败，没有关闭验证；PowerShell下载中断后续传完成。

`scripts/prepare-gshhg-land.py`读取官方高精度`gshhs_h.b`，按陆地/湖泊/湖中岛/岛内湖层级合成、截取116–136E/16–32N，以0.001度保拓扑简化。派生2996个多边形、29011个顶点，GeoJSON SHA256为`c4b7ffe170401cb91479cc5cb02a3d2a388c4861d524266dc3741eef2c322609`。原包与派生文件在`E:/CodexBuilds/harpoon-ai-reliability-20261021/map-source/`，许可证和说明保留于`data/geography/`。GSHHG是LGPLv3+，不是公共领域。

新想定复用项目已冻结的NASA Blue Marble2004-08影像，以同一WGS84输入、reference_lat和海里平面投影生成碰撞岸线。影像是旧合成背景，不是实时情报。没有宣称影像像素和概化岸线逐米吻合，更不把它当港口航道图。

指挥台公开轮廓由同一碰撞几何生成；复盘优先绘制档案内冻结的terrain，避免以后静态图层更新影响历史岸线。官方二进制包附LGPL、版权说明及README；其引用的GPLv3全文另从GNU官网保留到`GSHHG-COPYINGv3.txt`，不修改许可原文。

## 接入与验收要求

热开场新增全球WGS84 WMTS分级显示：缩放按需请求0–13级瓦片，6并发、256张内存缓存、失败退回NASA离线底图。实际请求服务与能力说明均为官方`tiles.maps.eox.at`；能力XML冻结在本轮map-source目录，`s2cloudless`明确标注2016与CC BY4.0。2016 & 2017署名文字按官方许可页保留；没有把旧拼接影像称为实时影像。来源在`web/maps/eox-source.json`。NASA离线新图3840×2880、114–138E/16–34N，其像素放大不等于传感器精度提高。

新热开场的碰撞海岸也独立截取到114–138E/16–34N，保留分层湖泊/岛屿，派生SHA256 `35f979a5d1e4f9d1af95da0de7e96b5c33e59667d177a2730ef9c7f58570938e`。在线影像覆盖可以继续平移到范围外；碰撞地形只对已冻结区域负责，不宣称全球避陆。旧版本仍使用原投影与档案内地形。

卫星图显示与碰撞多边形共享同一地理范围、WGS84输入及游戏投影；保存影像日期、许可、原始文件和派生子集SHA256。先比较指定区域的岸线差异，再决定是否用GSHHG替换Natural Earth；不能仅凭“高分辨率”字样宣称更准确。多边形需保留岛屿、湖泊和洞，检查海岸跨越、导航连通、出生点与旧档地理一致性。

海岸轮廓不能替代高程或水深，港口航道也不能从卫星像素直接推定。地形、海岸、影像分别维护。历史存档内冻结的海岸不随新版本暗中重写。

用户提出“北斗使蓝方有港内情报”：北斗的公开定位、导航、授时功能见[中国航天官方说明](https://www.cnsa.gov.cn/n6758823/n6758839/c6796158/content.html)。虚构演习可把“开局侦察资料”作为明确的剧本假设；不把北斗导航服务描述成持续侦察图像，也不因此给AI隐藏真态。
