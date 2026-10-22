"""Plot the saved M1d diagnostics; makes no model calls."""
from __future__ import annotations
import argparse
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt


def read(path):return json.loads(path.read_text(encoding="utf-8"))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage",required=True,type=Path)
    args=parser.parse_args()
    root=args.stage
    data=read(root/"stage-analysis.json")
    assert data["validation"]["status"]=="passed"
    fig,axes=plt.subplots(1,3,figsize=(13,3.5),layout="constrained")
    ax=axes[0]
    ax.bar([0,1],[data["dense_old_successes"],data["dense_new_successes"]],color=["#b4b4b4","#287b8e"])
    ax.set(xticks=[0,1],xticklabels=["Boolean edge","Revision rearming"],ylim=(0,9),ylabel="Completed tasks / 8",title="D5 scripted fault reproduction")
    for x,y in enumerate([data["dense_old_successes"],data["dense_new_successes"]]):ax.text(x,y+.2,str(y),ha="center")
    ax=axes[1]
    ax.bar([0,1],[data["expiry_old_calls"],data["expiry_new_calls"]],color=["#b4b4b4","#287b8e"])
    ax.set(xticks=[0,1],xticklabels=["No expiry gate","Public expiry gate"],ylim=(0,18),ylabel="Scripted requests across 8 episodes",title="D2 fixed grids: same task score (0)")
    for x,y in enumerate([data["expiry_old_calls"],data["expiry_new_calls"]]):ax.text(x,y+.3,str(y),ha="center")
    ax=axes[2]
    summary=read(root/"live-keyed-v2/summary.json")
    for row in summary:
        run=read(Path(row["trace"]))
        engine=read(Path(row["trace"]).parent/"engine.json")
        y={"eh":0,"em":1}[run["policy"]]
        for request in run["requests"]:
            left,right=request["requested_tick"],request["response_tick"]
            ax.plot([left,right],[y,y],color="#868686" if "stale" in request["status"] else "#287b8e",lw=7,solid_capstyle="butt")
            if request["applied_tick"] is not None:
                ax.scatter(request["applied_tick"],y,marker="|",s=150,color="black",zorder=3)
        if row["reached_tick"] is not None:ax.scatter(row["reached_tick"],y,marker="*",s=130,color="#ac7113",zorder=4)
    for update in engine["updates"]:ax.axvline(update["tick"],ls="--",color="#b94b4b",lw=1)
    ax.set(xlim=(0,700),ylim=(-.5,1.5),yticks=[0,1],yticklabels=["Human condition","Model condition"],xlabel="Simulation seconds (first 700 shown)",title="Real low-effort model, seed 7")
    ax.text(.02,.02,"bars: requests; gray: stale; |: applied; star: reached",transform=ax.transAxes,fontsize=7)
    for ax in axes:
        ax.spines[["top","right"]].set_visible(False)
        ax.tick_params(labelsize=8)
        ax.title.set_fontsize(10)
    output=root/"figures";output.mkdir(exist_ok=True)
    fig.savefig(output/"event-diagnostics.png",dpi=180)
    fig.savefig(output/"event-diagnostics.svg")
    plt.close(fig)
    print(str(output/"event-diagnostics.png"))


if __name__=="__main__":main()
