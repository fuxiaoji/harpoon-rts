/* Presentation only: never consult hidden entity state to resolve an enemy track. */
(function(root) {
  'use strict';
  // Two missed 4-second sensor passes in the current RTS client contract.
  // A display cue, not a new detection or weapon eligibility rule.
  function describe(contact, language='zh-CN') {
    const en=language==='en';
    const age=Math.max(0, Number(contact?.last_seen_s)||0);
    const lost=!!contact?.stale;
    const coasting=lost || age>=8;
    const seconds=Math.floor(age);
    const status=lost ? (en?'LOST':'失联') : coasting ? (en?'COAST':'外推') : (en?'TRACK':'跟踪');
    return {age,lost,coasting,status,
      label:coasting ? `${status} · ${seconds}${en?'s ago':'秒前'}` : status,
      explanation:coasting
        ? (en?'Position extrapolated from the last report; survival is unconfirmed.'
          :'位置按上次报告外推，尚未确认目标仍存活。')
        : (en?'Recently updated contact; position remains an estimate.':'近期有更新；接触位置仍是估计值。'),
    };
  }
  root.HarpoonContactTrack={describe};
  if(typeof module!=='undefined') module.exports=root.HarpoonContactTrack;
})(typeof window==='undefined'?globalThis:window);
