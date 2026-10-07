import fs from 'node:fs';
const dir = process.argv[2];
const files = fs.readdirSync(dir).filter(f=>f.endsWith('.ply')).sort();
function load(f){ const b=fs.readFileSync(dir+'/'+f); const h=b.indexOf('end_header\n')+11; const n=+/element vertex (\d+)/.exec(b.subarray(0,h).toString())[1];
  const set=new Set(); const keys=[]; let min=[1e9,1e9,1e9],max=[-1e9,-1e9,-1e9];
  for(let i=0;i<n;i++){const o=h+i*27; const x=b.readFloatLE(o),y=b.readFloatLE(o+4),z=b.readFloatLE(o+8);
    const k=b.toString('hex',o,o+12); set.add(k); for(const [j,v] of [x,y,z].entries()){if(v<min[j])min[j]=v;if(v>max[j])max[j]=v;}}
  return {n,set,min,max}; }
let prev=null, prevName='';
for(const f of files){ const c=load(f);
  let kept=0; if(prev){ for(const k of prev.set) if(c.set.has(k)) kept++; }
  console.log(f, 'n='+c.n, 'bbox', c.min.map(v=>v.toFixed(1)).join(','), '..', c.max.map(v=>v.toFixed(1)).join(','),
    prev? `prev_kept=${(kept/prev.n*100).toFixed(1)}% new=${c.n-kept}`:'');
  prev=c; }
