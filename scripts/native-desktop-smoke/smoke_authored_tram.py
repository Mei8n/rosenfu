#!/usr/bin/env python3
"""Ordinary authored street Tram: synthetic WebView input, exact native build."""
import argparse
import base64
import hashlib
import json
import math
from pathlib import Path
import tempfile

from fixture import RIGHT, config, point, projection, vector_lines
from smoke import FETCH_FIXTURE, MAP, SCREEN_POINT, pick
from smoke_tram import save
from wd_native import Driver, eventually, require

FIXTURE = Path(__file__).resolve().parents[2]/"crates/rosenfu-core/tests/fixtures/ofm_authored_tram_synthetic.json"
ASSET_REVISION="authored_tram_webview_16"
COMPANY="Synthetic authored Tram junction"
ORIGINAL_RAW_SHA="307c701215ad6bb460435e6b71eded53989998608370ce2697ae124047420839"
# Longer one-cell synthetic T with the same topology as the reviewed fixture;
# both tangent arms need room for the UI's minimum 40m platforms.
LONG_T=(((280,160),(780,160),(1280,160)),((780,160),(780,700)))
START=point(RIGHT,(360,160))
JOINT=point(RIGHT,(780,160))
STRAIGHT=point(RIGHT,(500,160))
END=point(RIGHT,(780,600))
STATIONS=(point(RIGHT,(520,160)),point(RIGHT,(780,460)))


def road_input(path=FIXTURE):
    proof=json.loads(path.read_text())
    require(hashlib.sha256(bytes(proof["raw"])).hexdigest()==proof["tileRawSha256"]==ORIGINAL_RAW_SHA,
            "unreviewed original authored Tram fixture")
    raw=vector_lines(LONG_T)
    out=config('single')
    template=out["template"].replace("rosenfu_webview_fixture_16",ASSET_REVISION)
    out["responses"]={key.replace("rosenfu_webview_fixture_16",ASSET_REVISION):value for key,value in out["responses"].items()}
    target=template.format(z=14,x=RIGHT[0],y=RIGHT[1])
    out["responses"][target]=base64.b64encode(raw).decode()
    out["template"]=template
    return out,target


def hover_on_map(w,at):
    x,y=w.sync(SCREEN_POINT,MAP,projection(at))
    w.command("POST","/actions",{"actions":[{"type":"pointer","id":"map-mouse","parameters":{"pointerType":"mouse"},
       "actions":[{"type":"pointerMove","duration":30,"origin":"viewport","x":round(x),"y":round(y)}]}]})


def assert_style(w,selector,prop,rgb):
    eventually(lambda:w.sync("""
      const e=document.querySelector(arguments[0]);
      return !!e&&getComputedStyle(e)[arguments[1]].replace(/\\s/g,'')===arguments[2];
    """,selector,prop,rgb))


def control(w,held):
    # W3C Control. Leave the pointer at the last road-guide preview location.
    w.command("POST","/actions",{"actions":[{"type":"key","id":"tram-smoke-keyboard","actions":[
        {"type":"keyDown" if held else "keyUp","value":"\ue009"}]}]})


def verify_green_and_ctrl(w):
    overlay='svg .tram-road-guide-overlay'
    preview='svg .tram-road-guide-preview'
    marker='svg .tram-road-guide-marker'
    assert_style(w,overlay,'stroke','rgb(41,135,70)')
    hover_on_map(w,STRAIGHT)
    assert_style(w,preview,'stroke','rgb(26,124,67)')
    assert_style(w,marker,'fill','rgb(26,124,67)')
    control(w,True)
    try:
        eventually(lambda:w.sync("""
          return !document.querySelector('.tram-road-guide-overlay,.tram-road-guide-preview,.tram-road-guide-marker')
            && !!document.querySelector('.candidate-draft')
            && document.querySelector('button[aria-label="道路に合わせる"]')?.getAttribute('aria-pressed')==='true';
        """))
    finally:
        control(w,False)
    # Refresh must happen on key release, without requiring another mouse move.
    assert_style(w,overlay,'stroke','rgb(41,135,70)')
    assert_style(w,preview,'stroke','rgb(26,124,67)')
    assert_style(w,marker,'fill','rgb(26,124,67)')


def distance_m(a,b):
    lat=(a[0]+b[0])/2
    return math.hypot((a[0]-b[0])*111_195,(a[1]-b[1])*111_195*math.cos(math.radians(lat)))


def assert_authored_shape(world):
    tracks=world.get('alignments',[])
    require(len(tracks)==1 and tracks[0]['track_type']=='tram',"native build didn't create a Tram alignment")
    line=tracks[0]; geo=[(p['latitude'],p['longitude']) for p in line['points']]
    require(len(geo)>3 and distance_m(geo[0],START)<4 and distance_m(geo[-1],END)<4,
            "native line endpoints don't match the two clicked road fractions")
    # A forbidden plain chord appreciably misses the physical road T node. A valid
    # fillet need not hit the node exactly but takes the two arms near it.
    require(min(distance_m(point,JOINT) for point in geo)<12,
            "native built shape omitted the road junction (straight fallback)")
    require(not world.get('road_transport',{}).get('tram_sections'),
            "normal guide incorrectly minted operational road authority")
    return line


def zoom_to_16(w):
    for z in range(13,17):
        w.sync("""window.dispatchEvent(new KeyboardEvent('keydown',
                {key:'+',code:'NumpadAdd',bubbles:true,cancelable:true}));""")
        eventually(lambda:w.sync("return [...document.querySelectorAll('.map-options span')].some(e=>e.textContent===arguments[0]);",
                                  f"ZL {z} / Web Mercator"))


def operating_stations(w,alignment_id):
    state=w.native('world_snapshot')
    places=[p for p in state.get('station_placements',[]) if p['alignment_id']==alignment_id]
    if len(places)!=2 or len(state['stations'])!=2:
        return None
    require(all(p['length_m']==40 for p in places),"station length changed or fitted elsewhere")
    centers={s['id']:(s['latitude'],s['longitude']) for s in state['stations']}
    ids=[]
    for wanted in STATIONS:
        sid=min(centers,key=lambda id:distance_m(centers[id],wanted))
        require(distance_m(centers[sid],wanted)<8 and any(p['station_id']==sid for p in places),
                "station didn't attach to the just-built T alignment")
        ids.append(sid)
    require(len(set(ids))==2,"stations collapsed or overlap")
    return ids,centers


def place_two_stations(w,alignment_id):
    w.click(w.action_element("//*[@role='tab' and normalize-space(.)='駅設置']"))
    w.input_css("#platform-length-cars","2")
    for i,pos in enumerate(STATIONS,1):
        hover_on_map(w,pos)
        eventually(lambda:w.sync("return !!document.querySelector('.station-placement-preview');"))
        pick(w,pos)
        eventually(lambda:w.sync("return document.querySelectorAll('g.planned-station').length===arguments[0];",i))
    w.click_text('計画を着工')
    return eventually(lambda:operating_stations(w,alignment_id),timeout=15)


def create_and_pause_service(w,alignment):
    stop_ids,centers=place_two_stations(w,alignment['id'])
    require(not w.native('operation_snapshot')['services'],"expected ordinary build with no timetable")
    w.click(w.action_element("//*[@role='tab' and normalize-space(.)='系統']"))
    w.click_text("新規系統")
    for sid in stop_ids:
        pos=centers[sid]
        x,y=w.sync(SCREEN_POINT,MAP,projection(pos))
        require(w.sync("return !!document.elementFromPoint(Math.round(arguments[0]),Math.round(arguments[1]))?.closest('.station-marker');",x,y),
                "map selection missed the actual authored station")
        pick(w,pos)
    w.click_text("編集終了")
    snapshot=eventually(lambda:(s if (s:=w.native('world_snapshot')).get('service_paths') else None))
    require(len(snapshot['services'])==1 and len(snapshot['service_paths'])==1,"unexpected native service")
    service=snapshot['services'][0]; sid=service['id']; path=snapshot['service_paths'][0]
    legs=[leg['segment'] for leg in path['legs']]
    require(service['start']==stop_ids[0] and path['service_id']==sid and len(legs)==1 and
            legs[0]['kind']=='alignment' and legs[0]['alignment_id']==alignment['id'] and
            [legs[0]['from'],legs[0]['to']]==stop_ids,"service did not traverse the exact just-built road-follow T")
    require(len(path['stops'])==2,"the two actual station stops were not both selected")
    label="系統編集: "+service['name']
    select=w.action_element(f"//button[@aria-label={json.dumps(label,ensure_ascii=False)}]")
    if w.sync("return arguments[0].getAttribute('aria-pressed')",select)!="true":
        w.click(select)
    w.click_text("運行を設定")
    eventually(lambda:any(c['service_id']==sid for c in w.native('operation_snapshot')['services']))
    w.click_text("再開")
    moving=eventually(lambda:next((t for t in w.native('operation_snapshot')['trains']
                                 if t['key']['service_id']==sid and t['state']['state']=='moving'),None),timeout=15)
    w.click_text("一時停止")
    paused=eventually(lambda:(s if (s:=w.native('operation_snapshot'))['paused'] else None))
    require(paused['now_ms']>0 and moving['key'] in [t['key'] for t in paused['trains']],"ordinary Tram was not paused while in service")
    return sid,path,stop_ids,moving['key'],paused['now_ms']


def run(w):
    engine=eventually(lambda:w.native('engine_info'))
    require(engine.get('authored_tram_road_guide_enabled') is True,"authored Tram road-guide capability missing")
    require(w.native('list_native_road_regions')==[],"normal track smoke must not use developer lab")
    w.input_css("#initial-company-name",COMPANY)
    w.click_text("この会社で開始")
    eventually(lambda:w.native("list_save_slots").get("active_slot_id"))
    w.click_text("地図・表示設定","summary")
    w.click_text("自作サンプル（オフライン）")
    w.click_text("地図・表示設定","summary")
    zoom_to_16(w)
    fixture,target=road_input()
    require(w.sync(FETCH_FIXTURE,fixture),"missing native bridge or WebCrypto")
    w.click_text('トラム')
    eventually(lambda:w.sync("""return [...document.querySelectorAll('button')].some(e=>
                    e.getAttribute('aria-label')==='道路に合わせる'&&e.getAttribute('aria-pressed')==='true');"""))
    eventually(lambda:target in w.sync("return window.__ordinaryBusNetworkSmoke"),timeout=15)
    eventually(lambda:w.sync("return [...document.querySelectorAll('[role=status]')].some(e=>e.textContent.includes('道路参考データ取得済み'));"),timeout=12)
    hover_on_map(w,START)
    eventually(lambda:w.sync("return !!document.querySelector('.road-snap-point')"),timeout=12)
    pick(w,START)
    eventually(lambda:w.sync("return document.querySelectorAll('.draft-anchor').length===1"))
    verify_green_and_ctrl(w)
    hover_on_map(w,END)
    eventually(lambda:w.sync("return document.body.innerText.includes('交差点を含む区間はクリック時に確認します');"))
    assert_style(w,"svg .tram-road-guide-marker","fill","rgb(26,124,67)")
    require(w.sync("return !document.querySelector('.confirmed-draft')"),"hover built a route")
    require(not w.native('world_snapshot').get('alignments'),"hover wrote native tracks")
    pick(w,END)
    eventually(lambda:w.sync("return !!document.querySelector('.confirmed-draft')"),timeout=15)
    require(not w.native('world_snapshot').get('alignments'),"planned route committed before construction")
    w.click_text("計画を着工")
    built=eventually(lambda:(s if (s:=w.native('world_snapshot')).get('alignments') else None))
    expected=assert_authored_shape(built)
    run=create_and_pause_service(w,expected)
    return save(w),expected,run


def reopen(w,saved_state):
    slot,expected,run=saved_state
    w.click_text("保存済みワールドを読み込む")
    w.click(w.action_element(f"//button[@aria-label={json.dumps(slot,ensure_ascii=False)}]"))
    actual=eventually(lambda:(s if (s:=w.native('world_snapshot')).get('alignments') else None))
    require(assert_authored_shape(actual)==expected,"native normal Tram changed after disk reopen")
    require(not w.sync("return !!window.__ordinaryBusNetworkSmoke"),"fresh process unexpectedly reused test HTTP scope")
    sid,path,stop_ids,key,millis=run
    places=operating_stations(w,expected['id'])
    require(any(p==path for p in actual['service_paths']) and places and places[0]==stop_ids,
            "the exact authored service and platforms didn't reopen")
    state=eventually(lambda:(s if (s:=w.native('operation_snapshot'))['services'] else None))
    require(state['paused'] and state['now_ms']==millis and any(t['key']==key for t in state['trains']) and
            any(config['service_id']==sid for config in state['services']) and not state['invalidated'],
            "the ordinary authored Tram active run/clock didn't reopen")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--driver',default='tauri-driver')
    p.add_argument('--logs',type=Path,required=True)
    opts=p.parse_args()
    opts.logs.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rosenfu-authored-tram-junction-') as temp:
        with Driver(opts.driver,Path(temp),opts.logs) as w:
            w.open(opts.app.resolve())
            receipt=run(w)
        with Driver(opts.driver,Path(temp),opts.logs) as w:
            w.open(opts.app.resolve())
            reopen(w,receipt)
    print("Native normal Tram: green guide and Ctrl verified; built the native synthetic T, placed two stations, ran the normal service, and restored its active train/clock.")

if __name__=='__main__':main()
