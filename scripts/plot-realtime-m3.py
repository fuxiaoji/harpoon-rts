"""Static publication-style diagnostic plots; no model calls."""
import argparse
import json
from pathlib import Path
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

def read(p):return json.loads(p.read_text(encoding="utf-8"))
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);a=p.parse_args();stage=a.stage
    output=stage/"figures";output.mkdir(exist_ok=True)
    timing=read(stage/"timing/validation.json")["summary"]
    analysis=read(stage/"quality/analysis.json")
    methods=["gf60","gf300","gf600","eh","em","ehq30","emcold"]
    fig,axes=plt.subplots(1,2,figsize=(11,4.2),layout="constrained")
    axes[0].bar(methods,[timing[m]["success"] for m in methods],color=["#8496a4"]*3+["#26858e"]*3+["#cc9b59"])
    axes[0].set(ylim=(0,17),ylabel="Completed / 16 scripted contexts",title="Controlled startup, stationary geometry")
    axes[0].tick_params(axis="x",rotation=35)
    axes[0].text(.02,.97,"100 sim-s reply delay; ready arms charged 1 preflight slot\nCold EM shown separately; no LLM calls",transform=axes[0].transAxes,va="top",fontsize=8)
    pairs=analysis["pairs"];labels=[p["tag"] for p in pairs];x=np.arange(len(pairs));width=.21
    for offset,(effort,color) in enumerate(zip(("low","medium","high"),("#7292a4","#d49a55","#237b83"))):
        axes[1].bar(x+(offset-1)*width,[p["scores"][effort]/p["optimum"] for p in pairs],width,label=effort,color=color)
    axes[1].plot(x,[p["greedy"]/p["optimum"] for p in pairs],"ko--",label="greedy",ms=3,lw=1)
    axes[1].annotate("high: no response\nSSE idle timeout",xy=(2+width,.01),xytext=(1.7,.28),fontsize=8,
        arrowprops={"arrowstyle":"->","color":"#a94f4f"},color="#a94f4f",ha="center")
    axes[1].axhline(1,color="#444444",lw=.8)
    axes[1].set(xticks=x,xticklabels=labels,ylim=(0,1.12),ylabel="Score / exact finite optimum",title="Real model: matched fixed snapshots")
    axes[1].tick_params(axis="x",rotation=35);axes[1].legend(ncol=4,fontsize=8,loc="upper center")
    fig.savefig(output/"stage-overview.png",dpi=180);fig.savefig(output/"stage-overview.svg");plt.close(fig)
    fig,axes=plt.subplots(1,2,figsize=(9,3.6),layout="constrained")
    for effort,color in zip(("low","medium","high"),("#7292a4","#d49a55","#237b83")):
        rs=[r for r in analysis["rows"] if r["effort"]==effort]
        axes[0].scatter([r["latency_s"] for r in rs],[r["normalized"] for r in rs],label=effort,color=color,s=40)
        for r in rs:
            d=__import__("math").ceil(r["latency_s"]*5)
            axes[1].plot([0,d],[r["scores"]["wide-0"],r["scores"].get(f"wide-{d}",0)],"o-",color=color,alpha=.7,lw=1)
    axes[0].set(xlabel="Client end-to-end latency (s)",ylabel="Zero-delay normalized score",title="Content quality and measured latency")
    axes[0].legend();axes[1].set(xlabel="Replayed delay (sim s, 5x)",ylabel="Actual navigation task score",title="Saved-output replay, not closed-loop")
    axes[0].annotate("SSE failure\nusage unknown",xy=(142.4082,0),xytext=(100,.25),fontsize=8,arrowprops={"arrowstyle":"->"})
    fig.savefig(output/"quality-latency.png",dpi=180);fig.savefig(output/"quality-latency.svg");plt.close(fig)
    fig,ax=plt.subplots(figsize=(6.4,3.8),layout="constrained")
    for effort,color in zip(("low","medium","high"),("#7292a4","#d49a55","#237b83")):
        row=next(r for r in analysis["rows"] if r["tag"]=="complex-11" and r["effort"]==effort)
        ax.plot([0,60,180],[row["scores"][f"wide-{d}"] for d in (0,60,180)],"o-",color=color,label=effort)
    bound=[r for r in read(stage/"delay-frontier/summary.json") if r["tag"]=="complex-11" and r["delay"] in (0,60,180)]
    ax.plot([r["delay"] for r in bound],[r["finite_optimum"] for r in bound],"k--",label="hindsight finite bound")
    ax.set(xlabel="Common injected delay (sim s)",ylabel="Actual navigation task score",xticks=[0,60,180],ylim=(0,46),
        title="Same zero-delay score, different temporal slack (complex-11)")
    ax.legend(fontsize=8);fig.savefig(output/"common-delay.png",dpi=180);fig.savefig(output/"common-delay.svg");plt.close(fig)
if __name__=="__main__":main()
