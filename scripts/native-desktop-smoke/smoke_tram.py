#!/usr/bin/env python3
"""True Dejima opt-in laboratory Tauri screen smoke. No mock IPC or network."""
import argparse
import json
import subprocess
import tempfile
from pathlib import Path

from wd_native import Driver, eventually, require

ASSET="dejima-osm-se-tram-lab"
DISPLAY_NAME="長崎・出島 南東向き試験区間（ゲーム内のみ）"
START,END="lab-dejima:se-clip-a","lab-dejima:se-clip-b"
EDGE="lab-dejima:osm-128993698-v12-17-18-se"
COMPANY="Dejima native track smoke"
PANEL="#panel-road-transport"


def native_world(w):
    return w.native("world_snapshot")


def await_native_lab(w):
    engine=eventually(lambda:w.native("engine_info"))
    require(engine.get("road_tram_enabled") and engine.get("operation_enabled"),"wrong engine capabilities")
    regions=eventually(lambda:w.native("list_native_road_regions"))
    row=next((row for row in regions if row['id']==ASSET),None)
    require(row and row.get('lab_only') is True and row.get('region')==DISPLAY_NAME,"exact labeled Dejima development catalog missing")


def open_lab(w):
    w.click(w.action_element("//*[@role='tab' and normalize-space(.)='道路交通（検証）']"))


def use_sample(w):
    w.click_text("地図・表示設定","summary")
    w.click_text("自作サンプル（オフライン）")
    eventually(lambda:w.element("css selector",'svg[aria-label="駅配置用サンプル地図"]'))
    w.click_text("地図・表示設定","summary")


def choose_directed_inset(w,source,target):
    # Dejima is a native lab graph, not a generic QA graph. The product shows
    # the source-geometry inset (map arrows are a separate synthetic-graph UI).
    for node in [source,target]:
        q=f'circle[aria-label="道路点 {node}"]'
        w.click(eventually(lambda:w.element("css selector",q)))
    require(w.label_control("起点","select"),"missing source selector")
    begin=w.label_control("起点","select")
    end=w.label_control("終点","select")
    require(w.sync("return arguments[0].value===arguments[2]&&arguments[1].value===arguments[3]",begin,end,source,target),
            "native input controls did not retain requested exact direction")


def build(w,source,target):
    choose_directed_inset(w,source,target)
    w.click_text("軌道を着工")


def save(w):
    tab=eventually(lambda:w.element('css selector','#workspace-tab-saves'))
    w.click(tab)
    w.click_text("ワールドを保存")
    eventually(lambda:w.sync("return document.body.innerText.includes('ワールドと未着工計画を保存しました。')"))
    state=w.native("list_save_slots")
    slot=next(row for row in state["slots"] if row['id']==state["active_slot_id"])
    require(slot['loadable'],"save slot not readable")
    return slot['name']


def load_slot(w,name):
    await_native_lab(w)
    w.click_text("保存済みワールドを読み込む")
    w.click(w.action_element(f"//button[@aria-label={json.dumps(name,ensure_ascii=False)}]"))
    eventually(lambda:w.native("list_save_slots").get("active_slot_id"))


def exact_clip(w,placement):
    world=native_world(w)
    data=world.get('road_transport',{})
    sections=data.get('tram_sections',[])
    if len(sections)!=1 or sections[0].get('placement',{}).get('kind')!=placement:return None
    section=sections[0]
    edge=next((edge for edge in data.get('edges',[]) if edge['id']==EDGE),None)
    line=next((line for line in world['alignments'] if line['id']==section['alignment_id']),None)
    require(edge and line and line['track_type']=='tram',"clip is not native Tram")
    require(line['points']==edge['geometry']==section['alignment_geometry'],"native edit moved the exact reference shape")
    require(section['from_position']==0 and section['to_position']==1,"not the entire exact clip")
    if placement=="shared_road":
        require(section['placement']['edges']==[EDGE],"shared road is not the pinned direction")
    return line,section


def create_edit(w):
    await_native_lab(w)
    w.input_css("#initial-company-name",COMPANY)
    w.click_text("この会社で開始")
    eventually(lambda:w.native("list_save_slots").get("active_slot_id"))
    use_sample(w)
    open_lab(w)
    w.click_text(DISPLAY_NAME+"を追加")
    roads=eventually(lambda:(r if (r:=native_world(w).get('road_transport',{})).get('edges') else None))
    require(len(roads['edges'])==1 and roads['edges'][0]['id']==EDGE and not roads['bus_stops'],"unexpected other road/pilot data")
    require(roads['edges'][0]['bus_access']!="allowed","lab tram must not authorize buses")
    build(w,END,START)
    eventually(lambda:w.sync("""const p=document.querySelector(arguments[0]);return [...p.querySelectorAll('[role=alert]')].some(a=>
       /道路経路|つながる経路/.test(a.textContent));""",PANEL))
    require(not native_world(w).get('alignments'),"native reverse refusal improperly wrote track")
    build(w,START,END)
    initial=eventually(lambda:exact_clip(w,"shared_road"))
    w.click_text("道路との関連を外す")
    detached=eventually(lambda:exact_clip(w,"dedicated"))
    require(detached[0]==initial[0],"detach changed identity/geometry")
    w.click_text("選択道路を関連付ける")
    require(eventually(lambda:exact_clip(w,"shared_road"))==initial,"reattach changed native shared clip")
    return save(w),initial


def reopen_build(w,expected):
    slot,clip=expected
    load_slot(w,slot)
    require(eventually(lambda:exact_clip(w,"shared_road"))==clip,"exact geometry/direction not recovered from disk")
    open_lab(w)
    w.text_action("道路との関連を外す")


def run_tram(w,fixture):
    load_slot(w,fixture['slot'])
    use_sample(w)
    line,section=eventually(lambda:exact_clip(w,"shared_road"))
    require(line['id']==fixture['clipId']==section['alignment_id'],"seed using a different clip")
    require(not w.native("operation_snapshot")['services'],"fixture should not start preconfigured")
    w.click(w.action_element("//*[@role='tab' and normalize-space(.)='系統']"))
    label="系統編集: "+fixture['service']
    w.click(w.action_element(f"//button[@aria-label={json.dumps(label,ensure_ascii=False)}]"))
    native=native_world(w)
    services=native['service_paths']
    rest=next(s for s in services if s['service_id']==fixture['serviceId'])
    segments=[row['segment'] for row in rest['legs']]
    require(len(segments)==1 and segments[0]['kind']=='connected_alignments',"seeded service is not a through path")
    ends={end['alignment_id'] for c in segments[0]['connections'] for end in [c['a'],c['b']]}
    require(fixture['clipId'] in ends and len(ends)==3,"service does not traverse exact clip and both approaches")
    w.click_text("運行を設定")
    eventually(lambda:any(c['service_id']==fixture['serviceId'] for c in w.native("operation_snapshot")['services']))
    w.click_text("再開")
    moving=eventually(lambda: next((t for t in w.native("operation_snapshot")['trains']
               if t['key']['service_id']==fixture['serviceId'] and t['state']['state']=='moving'),None),timeout=15)
    w.click_text("一時停止")
    paused=eventually(lambda: (s if (s:=w.native("operation_snapshot"))['paused'] else None))
    require(paused['now_ms']>0 and moving['key'] in [t['key'] for t in paused['trains']],"tram was not saved while running")
    return save(w),line,paused['now_ms'],moving['key']


def reopen_run(w,fixture,expected):
    slot,line,when,key=expected
    load_slot(w,slot)
    require(eventually(lambda:exact_clip(w,"shared_road"))[0]==line,"tram geometry changed on reopen")
    state=eventually(lambda: (s if (s:=w.native("operation_snapshot"))['services'] else None))
    require(state['paused'] and state['now_ms']==when and key in [t['key'] for t in state['trains']], "active tram/key/clock mismatch")
    w.select("速度","16")
    w.click_text("再開")
    end=eventually(lambda:(s if (s:=w.native("operation_snapshot"))['now_ms']>=60_000 and
                         not s['trains'] and not s['invalidated'] else None),timeout=16)
    require(end['now_ms']>=60_000,"resumed tram did not leave service at end")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',required=True,type=Path)
    parser.add_argument('--driver',default='tauri-driver')
    parser.add_argument('--seed',type=Path,help='exact source lab seed executable, for the separate through-operation rail fixture')
    parser.add_argument('--logs',required=True,type=Path)
    args=parser.parse_args()
    args.logs.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rosenfu-dejima-build-ui-') as tmp:
        root=Path(tmp)
        with Driver(args.driver,root,args.logs) as w:
            w.open(args.app.resolve());created=create_edit(w)
        with Driver(args.driver,root,args.logs) as w:
            w.open(args.app.resolve());reopen_build(w,created)
    print("LAB GUI: exact shared Tram created; reverse rejected; detach, reattach and reopen passed.")
    if args.seed:
        with tempfile.TemporaryDirectory(prefix='rosenfu-dejima-run-ui-') as tmp:
            root=Path(tmp)
            result=subprocess.run([str(args.seed.resolve()),str(root)],check=True,capture_output=True,text=True)
            fixture=json.loads(result.stdout)
            with Driver(args.driver,root,args.logs) as w:
                w.open(args.app.resolve());running=run_tram(w,fixture)
            with Driver(args.driver,root,args.logs) as w:
                w.open(args.app.resolve());reopen_run(w,fixture,running)
        print("LAB GUI: synthetic through service configured, running, paused, saved and resumed through clean completion.")
    else:
        print("LAB GUI operation phase not run; supply --seed.")

if __name__=='__main__':main()
