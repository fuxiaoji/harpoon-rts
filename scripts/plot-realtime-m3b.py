"""Standalone scientific figures for the frozen M3b development diagnostics."""
import argparse
import json
from pathlib import Path
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


def read(p): return json.loads(p.read_text(encoding="utf-8"))


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--stage",type=Path,required=True)
    parser.add_argument("--out",type=Path,required=True)
    a=parser.parse_args()
    a.out.mkdir(exist_ok=False)
    plt.rcParams.update({"font.family":"Microsoft YaHei","font.size":10,
                         "axes.spines.top":False,"axes.spines.right":False,
                         "svg.fonttype":"none"})
    rows=read(a.stage/"r1/summary.json")["groups"]
    colors={"EH":"#4a5568","Q30":"#d07844","Q-S":"#237f85",
            "GF60":"#7171b8","GF300":"#9b8aab","GF600":"#afabb7","Public":"#5c8c55"}
    arms=list(colors)
    fig,axes=plt.subplots(1,2,figsize=(11,4.5),sharey=True)
    for ax,sem,title in zip(axes,("replace","append"),("覆盖任务","追加任务")):
        for arm in arms:
            r=next(r for r in rows if r["response_s"]==100 and r["semantics"]==sem and r["arm"]==arm)
            ax.scatter(r["calls"],r["normalized_score_mean"],color=colors[arm],s=60)
            offset=(4,5 if arm!="Q-S" else -15)
            if sem=="append" and arm=="GF60": offset=(-28,-18)
            if sem=="append" and arm=="Q-S": offset=(5,7)
            ax.annotate(arm,(r["calls"],r["normalized_score_mean"]),xytext=offset,textcoords="offset points")
        ax.set(title=title,xlabel="16 格合计脚本调用次数",xlim=(-3,38),ylim=(0,1))
        ax.grid(alpha=.15)
    axes[0].set_ylabel("累计完成权重 / 发布权重（均值）")
    fig.suptitle("固定响应 100 秒：等待节省调用，也可能损失任务",fontsize=14)
    fig.text(.5,.015,"每种语义 16 个开发条件；Public 为暂停计算质量参照，CPU 另计。不是实际模型成本前沿。",ha="center",fontsize=9)
    fig.tight_layout(rect=(0,.045,1,.95))
    for ext in ("png","svg"): fig.savefig(a.out/f"wake-semantics.{ext}",dpi=180)
    plt.close(fig)

    info=read(a.stage/"r2/information-value.json")
    fig,ax=plt.subplots(figsize=(8.2,4.2))
    labels=["互斥分支","前置链","共享资源"]
    x=np.arange(3)
    for offset,rev,color,label in ((-.18,False,"#237f85","开始后不可撤销"),(.18,True,"#bca85d","开始后可返还（理想对照）")):
        vals=[next(r["gain"] for r in info if r["structure"]==family and r["slack_s"]==120 and r["reversible"]==rev)
              for family in ("branch","chain","shared")]
        bars=ax.bar(x+offset,vals,width=.34,color=color,label=label)
        ax.bar_label(bars,fmt="%.3f",padding=3)
    ax.set_xticks(x,labels)
    ax.set_ylabel("提前获得证据的期望价值增益")
    ax.set_ylim(0,max(b.get_height() for b in ax.patches)+1)
    ax.set_title("先知道更多，是否能改变承诺决策？")
    ax.legend(frameon=False,fontsize=9)
    fig.text(.5,.012,"三种 S2 按预测概率加权；零响应延迟，120 秒余量；三个固定开发模板。",ha="center",fontsize=9)
    fig.tight_layout(rect=(0,.04,1,1))
    for ext in ("png","svg"): fig.savefig(a.out/f"information-value.{ext}",dpi=180)
    plt.close(fig)
    pilot_path=a.stage/"r3-analysis-v2-20261023/summary.json"
    if pilot_path.exists():
        pilot=read(pilot_path)
        efforts=("low","medium","high")
        fig,axes=plt.subplots(1,2,figsize=(10.4,4.3))
        rr=pilot["rows"]
        gaps=[np.mean([r["risk_gap"] for r in rr if r["effort"]==e]) for e in efforts]
        bars=axes[0].bar(efforts,gaps,color=("#237f85","#bca85d","#d07844"),width=.6)
        axes[0].bar_label(bars,fmt="%.3f",padding=4)
        axes[0].set(ylabel="相对同信息有限风险参照的平均差距",title="首次内容：均匀0/60/180秒目标",ylim=(0,max(gaps)+.12))
        for i,e in enumerate(efforts):
            r=pilot["per_effort"][e]
            axes[0].text(i,.96,f'有效最优 {r["risk_optimal_valid"]}/{r["valid_plans"]}',
                         transform=axes[0].get_xaxis_transform(),ha="center",va="top",fontsize=9)
        durations=[[r["elapsed_s"] for r in rr if r["effort"]==e and r["status"]=="ok"] for e in efforts]
        axes[1].boxplot(durations,tick_labels=efforts,widths=.5,showfliers=True)
        axes[1].axhline(48,color="#b65a48",ls="--",lw=1,label="48墙钟秒＝240模拟秒")
        axes[1].set(ylabel="有效交付端到端耗时（秒）",title="交付耗时含客户端和网络")
        axes[1].legend(frameon=False,fontsize=8)
        fig.text(.5,.012,"12个固定条件格、3个模板；三次额度拒绝计入全请求后果，未作为模型思考耗时。非总体推断。",ha="center",fontsize=8)
        fig.tight_layout(rect=(0,.045,1,1))
        for ext in ("png","svg"): fig.savefig(a.out/f"reasoning-pilot.{ext}",dpi=180)
        plt.close(fig)
    print(a.out)


if __name__=="__main__": main()
