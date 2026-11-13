import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Segment = { kind: "track"; track_id: string } | { kind: "alignment"; alignment_id: string; from: string; to: string };
export type Candidate = { stations: string[]; segments: Segment[]; track_only: boolean };
export type Candidates = { status: "no_path" | "unique" | "ambiguous" | "incomplete"; candidates: Candidate[]; truncated: boolean };
export type Membership = { path: Candidate; member_stations: string[]; excluded_stations: string[] };
type Props = {
  stations: { id: string; name: string }[];
  worldVersion: unknown;
  resetKey?: unknown;
  onDirty?: (dirty: boolean) => void;
  enabled: boolean;
  canApply: boolean;
  onApply: (start: string, tracks: string[]) => void;
  onSave?: (name: string, membership: Membership) => Promise<void>;
};

/** Candidate planner; optional persistence callback saves physical path and membership separately. */
export function RouteCandidatePreview({ stations, worldVersion, resetKey, onDirty, enabled, canApply, onApply, onSave }: Props) {
  const [name, setName] = useState("新路線");
  const [saving, setSaving] = useState(false);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [result, setResult] = useState<Candidates | null>(null);
  const [chosen, setChosen] = useState<number | null>(null);
  const [membership, setMembership] = useState<Membership | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [stale,setStale]=useState(false);
  const generation = useRef(0);
  const pending = useRef(false);
  const names = new Map(stations.map(station => [station.id, station.name]));
  const stationName = (id: string) => names.get(id) ?? `駅 ${id}`;
  function clear() {
    generation.current++;
    pending.current = false;
    setStale(false); setLoading(false); setResult(null); setChosen(null); setMembership(null); setError("");
  }
  function discard() { clear(); setStart("");setEnd("");setName("新路線"); }
  useEffect(() => { discard(); }, [resetKey]);
  useEffect(() => {
    generation.current++; pending.current=false; setLoading(false);
    // Preserve the user's endpoints/exclusions, but never apply stale geometry.
    setStale(true);
    return () => { generation.current++; };
  }, [worldVersion]);
  useEffect(()=>{onDirty?.(!!start || !!end || !!result || name!=="新路線");},[start,end,result,name,onDirty]);
  async function validate(candidate: Candidate, excluded: string[], token: number) {
    const preview = await invoke<Membership>("preview_route_membership", { start: candidate.stations[0], segments: candidate.segments, excludedStations: excluded });
    if (token === generation.current) setMembership(preview);
  }
  async function search() {
    if (!enabled || pending.current || !names.has(start) || !names.has(end) || start === end) return;
    clear();
    pending.current = true; setLoading(true);
    const token = generation.current;
    try {
      const response = await invoke<Candidates>("preview_route_candidates", { start, end });
      if (token !== generation.current) return;
      setResult(response);
      // Incomplete or ambiguous searches always require an explicit selection.
      if (response.status === "unique" && !response.truncated && response.candidates.length === 1) {
        setChosen(0);
        await validate(response.candidates[0], [], token);
      }
    } catch (cause) {
      if (token === generation.current) setError(String(cause));
    } finally {
      if (token === generation.current) { pending.current = false; setLoading(false); }
    }
  }
  async function selectCandidate(index: number, excluded: string[] = []) {
    if (!enabled || pending.current || !result?.candidates[index]) return;
    const token = ++generation.current;
    pending.current = true; setLoading(true); setError(""); setChosen(index); setMembership(null);
    try { await validate(result.candidates[index], excluded, token); }
    catch (cause) { if (token === generation.current) setError(String(cause)); }
    finally { if (token === generation.current) { pending.current = false; setLoading(false); } }
  }
  const candidate = chosen === null ? null : result?.candidates[chosen];
  const importable = membership && membership.path.track_only && membership.excluded_stations.length === 0
    && membership.path.segments.every(segment => segment.kind === "track");
  return <section className="route-candidate-preview" aria-label="起終点から経路候補">
    <h4>起終点から経路候補</h4>
    <p className="field-hint">{onSave ? "所属駅の除外は設備削除・列車の通過指定とは別です。新規路線へ適用後、ワールドを保存してください。" : "候補と所属駅のプレビューです。駅の除外は設備の削除・列車の通過指定ではなく、まだ保存されません。"}</p>
    <fieldset disabled={!enabled}>
      <label htmlFor="candidate-start">候補の起点</label>
      <select id="candidate-start" value={start} onChange={event => { clear(); setStart(event.target.value); }}><option value="">駅を選択</option>{stations.map(station => <option key={station.id} value={station.id}>{station.name} (ID {station.id})</option>)}</select>
      <label htmlFor="candidate-end">候補の終点</label>
      <select id="candidate-end" value={end} onChange={event => { clear(); setEnd(event.target.value); }}><option value="">駅を選択</option>{stations.map(station => <option key={station.id} value={station.id}>{station.name} (ID {station.id})</option>)}</select>
      <button type="button" disabled={!enabled || loading || !names.has(start) || !names.has(end) || start === end} onClick={() => void search()}>経路候補を検索</button>
      <button type="button" className="secondary" disabled={!result && !loading && !error && !start && !end && name==="新路線"} onClick={discard}>候補を閉じる</button>
    </fieldset>
    {stale && result && <p role="status">ワールドが変更されました。候補は保持していますが、適用前に経路候補を再検索してください。</p>}
    {loading && <p role="status">経路を確認中…</p>}
    {error && <p role="alert">候補の確認に失敗しました: {error}</p>}
    {result && <>
      {(result.status === "incomplete" || result.truncated) ? <p role="status">探索上限に達しました。候補は一部のみで、唯一の経路・接続なしとは判定できません。</p>
        : result.status === "no_path" ? <p role="status">現在の接続関係では経路が見つかりません。交差・近接だけでは線路は接続されません。</p>
        : result.status === "ambiguous" ? <p role="status">複数の経路があります。使用する候補を選んでください。</p>
        : <p role="status">経路候補が見つかりました。</p>}
      <p className="field-hint">候補順は距離・速さのおすすめ順ではありません。環状経路は手動編集を使ってください。</p>
      <fieldset disabled={!enabled || loading}><legend>経路候補</legend>
        {result.candidates.map((item, index) => <label key={index} className="candidate-choice"><input type="radio" name="route-candidate" checked={chosen === index} onChange={() => void selectCandidate(index)}/>{`候補 ${index + 1}: ${item.stations.map(stationName).join(" → ")}`}<small>{item.track_only ? "駅間接続のみ" : "建設線形を含む"}</small></label>)}
      </fieldset>
    </>}
    {candidate && membership && <>
      {onSave && <><label htmlFor="planned-route-name">候補から作る路線名</label><input id="planned-route-name" value={name} maxLength={80} disabled={saving} onChange={event=>setName(event.target.value)}/><button type="button" disabled={!enabled || stale || loading || saving || !canApply || !name.trim()} onClick={async()=>{
        if (!enabled || stale || pending.current || saving || !canApply || !name.trim()) return;
        pending.current=true;setSaving(true);setError("");
        try { await onSave(name.trim(),membership); discard(); } catch(cause) { setError(String(cause)); } finally {pending.current=false;setSaving(false);}
      }}>候補を新規路線へ適用</button>{!canApply && <p>先に手動路線の下書きを適用または破棄してください。</p>}</>}
      <fieldset disabled={!enabled || loading}><legend>候補の所属駅（未保存）</legend>
        {membership.path.stations.map((id, index, all) => <label key={id} className="candidate-choice"><input type="checkbox" checked={membership.member_stations.includes(id)} disabled={index === 0 || index === all.length - 1} onChange={event => {
          const excluded = event.target.checked ? membership.excluded_stations.filter(item => item !== id) : [...membership.excluded_stations, id];
          void selectCandidate(chosen!, excluded);
        }}/>{stationName(id)}{index === 0 ? "（起点）" : index === all.length - 1 ? "（終点）" : ""}</label>)}
      </fieldset>
      <p>所属駅: {membership.member_stations.map(stationName).join(" → ")}</p>
      {!onSave && !importable && <p className="field-hint">建設線形を含む経路、または駅を除外した所属一覧の保存は未対応です。この候補はプレビューのみです。</p>}
      {importable && <>
        <button type="button" disabled={!enabled || stale || loading || !canApply} onClick={() => {
          if (!enabled || stale || pending.current || !canApply) return;
          onApply(membership.path.stations[0], membership.path.segments.flatMap(segment => segment.kind === "track" ? [segment.track_id] : []));
          discard();
        }}>新規路線の手動下書きへ使う</button>
        {!canApply && <p className="field-hint">先に編集中の路線下書きを適用または破棄してください。</p>}
        <p className="field-hint">取り込みだけでは保存されません。下の路線名と経路を確認し「路線を適用」を押してください。</p>
      </>}
    </>}
  </section>;
}
