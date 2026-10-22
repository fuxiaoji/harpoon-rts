"""Static research plots from audited M2a data; no model calls."""
import argparse
import itertools
import json
from pathlib import Path
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.colors import ListedColormap
import numpy as np


def read(path):return json.loads(path.read_text(encoding="utf-8"))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--stage",type=Path,required=True);args=p.parse_args();root=args.stage
    output=root/"figures";output.mkdir(exist_ok=True)
    scripted=read(root/"scripted/analysis.json");assert scripted["status"]=="passed"
    contexts=list(itertools.product((59,239),(30,180),(420,600)))
    labels=[f"p{p}\ng{g}/w{w}" for p,g,w in contexts]
    methods=["gf60","gf300","gf600","eh","em","hh600","hm600","rawf60","rawf300","rawf600"]
    fig,axes=plt.subplots(1,2,figsize=(12,6),layout="constrained")
    for ax,delay in zip(axes,(100,150)):
        data=[];text=[]
        for method in methods:
            values=[];annotations=[]
            for phase,gap,window in contexts:
                rs=[r for r in scripted["episodes"] if r["method"]==method and r["budget"]==6 and r["delay"]==delay and
                    (r["parameters"]["first_event_tick"],r["parameters"]["gap_s"],r["parameters"]["event_window_s"])==(phase,gap,window)]
                assert len(rs)==2
                values.append(sum(r["success"] for r in rs)/2)
                annotations.append(f"{sum(r['success'] for r in rs)}/2\n{sum(r['requests'] for r in rs)/2:g} req")
            data.append(values);text.append(annotations)
        ax.imshow(data,cmap=ListedColormap(["#eeeeee","#8ec5c5","#287b8e"]),vmin=0,vmax=1,aspect="auto")
        ax.set(xticks=range(8),xticklabels=labels,yticks=range(len(methods)),yticklabels=methods,
               title=f"Injected reply delay {delay} sim s; cap 6")
        for y,row in enumerate(text):
            for x,t in enumerate(row):ax.text(x,y,t,ha="center",va="center",fontsize=7,color="white" if data[y][x]==1 else "black")
        ax.tick_params(length=0,labelsize=8)
    fig.suptitle("Scripted controls: completions / 2 geometry seeds; mean requests (not LLM calls)",fontsize=11)
    fig.savefig(output/"scripted-factorial.png",dpi=180);fig.savefig(output/"scripted-factorial.svg");plt.close(fig)
    if not (root/"clean-analysis.json").exists():return
    live=read(root/"clean-analysis.json");assert live["status"]=="passed"
    methods=["gf300","eh","em","public"]
    fig,(ax,cost)=plt.subplots(1,2,figsize=(11,5),layout="constrained",gridspec_kw={"width_ratios":[1.5,1]})
    values=[];annotations=[]
    for phase,gap,window in contexts:
        row=[];text=[]
        for method in methods:
            r=next(r for r in live["episodes"] if r["method"]==method and
                (r["parameters"]["first_event_tick"],r["parameters"]["gap_s"],r["parameters"]["event_window_s"])==(phase,gap,window))
            row.append(int(r["success"]));text.append(f"{'done' if r['success'] else 'miss'}\n{r['model_calls']} LLM calls")
        values.append(row);annotations.append(text)
    ax.imshow(values,cmap=ListedColormap(["#eeeeee","#287b8e"]),vmin=0,vmax=1,aspect="auto")
    ax.set(xticks=range(4),xticklabels=methods,yticks=range(8),yticklabels=[f"phase={p}, gap={g}, window={w}" for p,g,w in contexts],
           title="Real low-effort pilot: one run per cell")
    for y,row in enumerate(annotations):
        for x,t in enumerate(row):ax.text(x,y,t,ha="center",va="center",fontsize=8,color="white" if values[y][x] else "black")
    ax.tick_params(length=0,labelsize=8)
    for method in methods:
        rs=[r for r in live["episodes"] if r["method"]==method]
        x=sum(r["model_calls"] for r in rs);y=sum(r["success"] for r in rs)
        cost.scatter(x,y,s=65,label=method)
        cost.annotate(method,(x,y),xytext=(5,5),textcoords="offset points",fontsize=9)
    cost.set(xlabel="Actual LLM calls across 8 temporal contexts",ylabel="Completed final tasks / 8",ylim=(-.5,8.8),title="Observed effect and call cost")
    cost.spines[["top","right"]].set_visible(False);cost.grid(alpha=.2)
    fig.suptitle("Development grid, geometry seed 7; no population confidence or universal ranking",fontsize=11)
    fig.savefig(output/"real-factorial.png",dpi=180);fig.savefig(output/"real-factorial.svg");plt.close(fig)
    print(str(output))


if __name__=="__main__":main()
