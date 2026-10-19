(function(root){
  'use strict';
  function roles(platform,weapons) {
    const fits=(platform?.weapons||[]).map(f=>weapons.get(f.weapon)).filter(Boolean);
    const asw=!!platform?.dipping_sonar || fits.some(w=>w.targets?.sub);
    const cap=fits.some(w=>w.targets?.air);
    // The current strike mission releases ASCMs; do not promise unsupported land attacks.
    const strike=fits.some(w=>String(w.kind).toLowerCase()==='ascm' && w.targets?.surface);
    return {primary:asw?'asw':cap?'cap':'recon',strike};
  }
  root.HarpoonAircraft={roles};
  if(typeof module!=='undefined')module.exports={roles};
})(typeof window!=='undefined'?window:globalThis);
