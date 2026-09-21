import React from 'react';
import { createRoot } from 'react-dom/client';
import './apps/www/app/globals.css';
import { BotCreatureAvatar } from './apps/www/components/app/bot-creature-avatar';
function Preview() {
  const [active, setActive] = React.useState(true); const [debug, setDebug] = React.useState(''); React.useEffect(() => { const p = document.querySelector('.creature-particle'); if (p) { const c = getComputedStyle(p); const a = getComputedStyle(p, '::after'); setDebug(JSON.stringify({display:c.display, animation:c.animation, transform:c.transform, color:c.color, width:a.width, height:a.height, content:a.content, background:a.backgroundColor, position:a.position, left:a.left, top:a.top, opacity:c.opacity, visibility:c.visibility, z:c.zIndex, rect:p.getBoundingClientRect().toJSON()})); } }, []);
  return <main className="min-h-screen bg-background p-12 text-foreground"><h1 className="mb-6 text-xl">Working avatar preview</h1><p>{debug}</p><p>Reduced motion: {String(matchMedia('(prefers-reduced-motion: reduce)').matches)}</p><button className="mb-8 rounded border p-2" onClick={() => setActive(!active)}>{active ? 'Stop responding' : 'Start responding'}</button>{[false, true].map(dark => <section key={String(dark)} className={`${dark ? 'dark' : ''} mb-4 rounded-xl bg-background p-8 text-foreground ring-1 ring-border`}><div className="flex items-center gap-14">{(['xs','sm','md','lg','xl','2xl'] as const).map(size => <div key={size} className="flex flex-col items-center gap-5"><BotCreatureAvatar name="Engineer" avatarId="teal-wisp" src="/apps/www/public/marketing/job-bots/engineer.png" size={size} animated={active} /><small>{size}</small></div>)}</div></section>)}</main>;
}
createRoot(document.getElementById('root')!).render(<Preview/>);
