import { useRef, useState } from 'react';

export function ServiceDelete({name, enabled, hasEdits, onDelete}: {
  name: string; enabled: boolean; hasEdits: boolean; onDelete: () => Promise<void>;
}) {
  const [confirm,setConfirm]=useState(false);
  const [pending,setPending]=useState(false);
  const [error,setError]=useState('');
  const locked=useRef(false);
  async function remove(){
    if(!enabled||hasEdits||locked.current)return;
    locked.current=true;setPending(true);setError('');
    try{await onDelete();setConfirm(false);}catch(e){setError(String(e));}
    finally{locked.current=false;setPending(false);}
  }
  return <div>
    <button type="button" className="secondary" disabled={!enabled||hasEdits||pending} onClick={()=>{setConfirm(true);setError('');}}>系統を削除</button>
    {hasEdits&&<p className="field-hint">編集中の変更を適用または取り消すと削除できます。</p>}
    {confirm&&<div className="inline-confirm" role="dialog" aria-label="系統削除の確認">
      <p>「{name}」を削除しますか。線路・駅・路線と他の系統は残ります。</p>
      <button type="button" disabled={pending} onClick={()=>setConfirm(false)}>キャンセル</button>
      <button type="button" disabled={!enabled||hasEdits||pending} onClick={()=>void remove()}>削除する</button>
      {error&&<p role="alert">{error}</p>}
    </div>}
  </div>;
}
