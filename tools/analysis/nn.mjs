import fs from 'node:fs';
const dir=process.argv[2];
function load(f){const b=fs.readFileSync(dir+'/'+f);const h=b.indexOf('end_header\n')+11;const n=+/element vertex (\d+)/.exec(b.subarray(0,h).toString())[1];
 const p=new Float32Array(n*3);for(let i=0;i<n;i++){const o=h+i*27;p[i*3]=b.readFloatLE(o);p[i*3+1]=b.readFloatLE(o+4);p[i*3+2]=b.readFloatLE(o+8);}return {n,p};}
function grid(c,s){const m=new Map();for(let i=0;i<c.n;i++){const k=Math.floor(c.p[i*3]/s)+','+Math.floor(c.p[i*3+1]/s)+','+Math.floor(c.p[i*3+2]/s);let a=m.get(k);if(!a)m.set(k,a=[]);a.push(i);}return m;}
function nn(a,b,s){const g=grid(b,s);const d=[];const step=Math.max(1,Math.floor(a.n/20000));
 for(let i=0;i<a.n;i+=step){const x=a.p[i*3],y=a.p[i*3+1],z=a.p[i*3+2];let best=Infinity;const cx=Math.floor(x/s),cy=Math.floor(y/s),cz=Math.floor(z/s);
  for(let dx=-1;dx<=1;dx++)for(let dy=-1;dy<=1;dy++)for(let dz=-1;dz<=1;dz++){const L=g.get((cx+dx)+','+(cy+dy)+','+(cz+dz));if(!L)continue;for(const j of L){const e=(b.p[j*3]-x)**2+(b.p[j*3+1]-y)**2+(b.p[j*3+2]-z)**2;if(e<best)best=e;}}
  d.push(Math.sqrt(best));}
 d.sort((u,v)=>u-v);const q=t=>d[Math.floor(t*(d.length-1))];return `p10=${q(.1).toFixed(4)} p50=${q(.5).toFixed(4)} p90=${q(.9).toFixed(4)} beyond(0.5m)=${(d.filter(v=>v>0.5).length/d.length*100).toFixed(1)}%`;}
const files=fs.readdirSync(dir).filter(f=>f.endsWith('.ply')).sort();
const pairs=[[1,3],[3,5],[5,7],[11,13],[0,1]];
for(const [a,b] of pairs){const A=load(files[a]),B=load(files[b]);console.log(files[a],'->',files[b],'| prev pts NN in next:',nn(A,B,0.5),'| next pts NN in prev:',nn(B,A,0.5));}
// self spacing reference
const R=load(files[3]); console.log('self-spacing ref (refined_1 vs itself excl? approx via preview):');
