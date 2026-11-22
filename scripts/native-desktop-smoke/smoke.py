#!/usr/bin/env python3
"""Ordinary Bus actual Tauri + WebKit smoke, no feature flag or installer."""
import argparse
import json
import os
from pathlib import Path
import tempfile

from fixture import CAMERA, config, projection, REVISION
from wd_native import Driver, eventually, nav_click_payload, require

NAME = "Ordinary Bus synthetic smoke"
MAP = 'svg[role="img"][aria-label^="駅配置用"]'
PANEL = '#panel-ordinary-bus'

# Replaces NETWORK response bodies only, in this one temporary WebView.
# No interception of invoke, worker, React state, persisted cache or disk.
FETCH_FIXTURE = """
const fixture=arguments[0], original=window.fetch.bind(window), read=[];
if(window.__ordinaryBusNetworkSmoke)throw Error('fixture already installed');
window.__ordinaryBusNetworkSmoke=read;
window.fetch=function(input,options){
  const url=typeof input==='string'?input:input?.url ?? input?.href;
  if(url===fixture.meta){
    read.push(url);
    return Promise.resolve(new Response(JSON.stringify({maxzoom:14,tiles:[fixture.template]}),
                           {headers:{'Content-Type':'application/json'}}));
  }
  if(Object.hasOwn(fixture.responses,url)){
    read.push(url);
    const raw=Uint8Array.from(atob(fixture.responses[url]),c=>c.charCodeAt(0));
    return Promise.resolve(new Response(raw,{headers:{'Content-Type':'application/x-protobuf'}}));
  }
  return original(input,options);
};
return Boolean(window.__TAURI_INTERNALS__?.invoke)&&Boolean(globalThis.crypto?.subtle);
"""

SCREEN_POINT = """
const svg=document.querySelector(arguments[0]);
if(!svg)throw Error('ordinary map not mounted');
const ctm=svg.getScreenCTM();
if(!ctm)throw Error('SVG transform missing');
const screen=new DOMPoint(...arguments[1]).matrixTransform(ctm);
const hit=document.elementFromPoint(Math.round(screen.x),Math.round(screen.y));
if(hit?.closest('svg[role="img"]')!==svg)throw Error('map fixture vertex is clipped or occluded');
return [screen.x,screen.y];
"""

def open_bus(w):
    w.click(w.action_element("//*[@role='tab' and normalize-space(.)='バス']"))


def close_map_controls(w):
    w.click_text("地図・表示設定","summary")


def setup(w,scenario):
    engine=eventually(lambda:w.native("engine_info"))
    require(engine.get("ordinary_ofm_bus_enabled") is True,"ordinary Bus capability absent")
    if scenario=="loop":
        require(engine.get("ordinary_ofm_bus_closed_loop_enabled") is True,"closed-loop capability absent")
    if scenario=="seam":
        require(engine.get("ordinary_ofm_bus_rectangle_enabled") is True,"ordinary Bus rectangle capability absent")
    require(engine.get("ordinary_ofm_bus_multiple_enabled") is True,"indexed ordinary Bus capability absent")
    require(engine.get("ordinary_ofm_bus_round_trip_enabled") is True,"native ordinary Bus return capability absent")
    require(engine.get("ordinary_ofm_bus_waypoints_enabled") is True,"ordinary Bus waypoint editor capability absent")
    require(engine.get("ordinary_ofm_bus_interior_stops_enabled") is True and engine.get("bus_operation_enabled") is True,"ordinary Bus stop/operation capability absent")
    require(w.native("list_native_road_regions")==[],"app includes a debug road catalog")
    w.input_css("#initial-company-name",NAME)
    w.click_text("この会社で開始")
    eventually(lambda:w.native("list_save_slots").get("active_slot_id"))
    close_map_controls(w)
    w.click_text("自作サンプル（オフライン）")
    eventually(lambda:w.element("css selector",'svg[aria-label="駅配置用サンプル地図"]'))
    close_map_controls(w)

def pick(w,at):
    x,y=w.sync(SCREEN_POINT,MAP,projection(at))
    w.command("POST","/actions",nav_click_payload(x,y))
    w.command("DELETE","/actions")


def wait_visible(w, css):
    return eventually(lambda:w.sync("return [...document.querySelectorAll(arguments[0])].find(e=>e.getClientRects().length)||null",css))


def wait_panel_text(w, text):
    return eventually(lambda:w.sync("""return Boolean(document.querySelector(arguments[0])?.innerText.includes(arguments[1]));""",PANEL,text))


def press_escape(w):
    # W3C Escape key (no JS listener invocation).
    w.command("POST","/actions",{"actions":[{"type":"key","id":"smoke-keyboard","actions":[
        {"type":"keyDown","value":"\ue00c"},{"type":"keyUp","value":"\ue00c"}]}]})
    w.command("DELETE","/actions")


def verify_existing_edit(w,last,route_id,binding):
    w.click_text("バス系統を引き直す")
    endpoint_css='circle[aria-label="バスの終点"]'
    wait_visible(w,endpoint_css)
    preview=wait_visible(w,'path[aria-label="バス系統のプレビュー"]')
    path_d=w.sync("return arguments[0].getAttribute('d')",preview)
    idle='線や点をドラッグして編集'
    extending='道路を順にクリック'
    wait_panel_text(w,idle)
    # The known centre is well off both synthetic endpoint controls and the
    # preview. An ordinary blank click must not append or enter extension.
    pick(w,CAMERA[:2])
    unchanged=w.sync("""
      const panel=document.querySelector(arguments[0]), preview=document.querySelector('path[aria-label="バス系統のプレビュー"]');
      const controls=document.querySelectorAll('circle[data-bus-control]');
      return controls.length===2&&panel?.innerText.includes(arguments[1])&&
        !panel?.innerText.includes(arguments[2])&&preview?.getAttribute('d')===arguments[3]&&
        !panel.querySelector('[role="alert"]');
      """,PANEL,idle,extending,path_d)
    require(unchanged,"blank click altered saved-route edit or entered extension")
    # Hit an actual DOM endpoint handle through the projected SVG screen.
    x,y=w.sync(SCREEN_POINT,MAP,projection(last['geo']))
    require(w.sync("return document.elementFromPoint(Math.round(arguments[0]),Math.round(arguments[1]))?.matches(arguments[2]);",x,y,endpoint_css),
            "saved endpoint handle does not correspond to the native path endpoint")
    pick(w,last['geo'])
    wait_panel_text(w,extending)
    press_escape(w)
    wait_panel_text(w,idle)
    require(w.sync("return document.querySelectorAll('circle[data-bus-control]').length===2"),"first Escape closed or changed the edited draft")
    press_escape(w)
    w.text_action("バス系統を引き直す")
    require(proofs(snapshot(w))=={route_id:binding},"draft gestures modified the committed native route")


STOP_NAME="Synthetic centre stop"

def stop_edit(w,route_id,binding):
    w.click_text("停留所を編集")
    path=wait_visible(w,'g[aria-label="停留所を置くバス経路"] > path')
    point=w.sync("""
      const p=arguments[0], c=p.getScreenCTM();
      if(!c)throw Error('missing stop pick transform');
      const at=p.getPointAtLength(p.getTotalLength()*.37).matrixTransform(c);
      const hit=document.elementFromPoint(Math.round(at.x),Math.round(at.y));
      if(hit?.closest('svg[role="img"]')!==p.ownerSVGElement)throw Error('manual stop is clipped');
      return [at.x,at.y];""",path)
    w.command("POST","/actions",nav_click_payload(*point))
    w.command("DELETE","/actions")
    field=w.label_control("停留所名 1","input")
    w.input_element(field,STOP_NAME)
    require(proofs(snapshot(w))=={route_id:binding},"unconfirmed stop changed native route")
    w.click_text("停留所の編集を確定")
    updated=eventually(lambda: (p if (p:=proofs(snapshot(w)).get(route_id)) and p.get('interiorStops') else None))
    interior=updated['interiorStops']
    require(len(interior)==1 and interior[0]['name']==STOP_NAME and interior[0]['occurrenceKey'],"native stop proof/name not retained")
    require(len(snapshot(w)['road_transport']['bus_stops'])==3,"native route does not have two end stops plus intermediate")
    return updated


def drive_and_pause(w,route_id):
    w.click_text("運行を設定")
    eventually(lambda:any(row['route_id']==route_id for row in w.native('operation_snapshot').get('bus_services',[])))
    w.click_text("再開")
    moving=eventually(lambda:next((t for t in w.native('operation_snapshot')['trains']
                                  if t['key']['service_id']==f'bus:{route_id}' and t['state']['state']=='moving'),None),
                      timeout=15)
    w.click_text("一時停止")
    paused=eventually(lambda: (p if (p:=w.native('operation_snapshot'))['paused'] else None))
    require(paused['now_ms']>0 and moving['key'] in [t['key'] for t in paused['trains']],"the synthetic bus was not paused during a real run")
    return moving['key'],paused['now_ms']


def snapshot(w):
    return w.native("world_snapshot")


def proofs(world):
    found=world.get('ofm_gameplay_bus_routes',{})
    require(isinstance(found,dict),f"indexed Bus evidence is not a map: {found!r}")
    return found


def initiate(w,scenario):
    setup(w,scenario)
    fixture=config(scenario)
    require(w.sync(FETCH_FIXTURE,fixture),"missing native bridge or WebCrypto in real WebView")
    require(not proofs(snapshot(w)),"fresh profile already has a bus")
    open_bus(w)
    w.click_text("新しいバス系統")
    # This exact candidate auto-zooms ordinary New/Edit to 16.
    require(w.sync("return !![...document.querySelectorAll('.map-options span')].find(e=>e.textContent==='ZL 16 / Web Mercator');"),
            "fixture expects the normal editor's ZL16")
    w.select("停留所の配置","manual")
    primary=fixture["primaryShas"]
    eventually(lambda:set(w.sync("return window.__ordinaryBusNetworkSmoke")).issuperset([fixture["meta"],*primary]),timeout=15)
    # Observe loader settling, then make actual W3C pointer clicks. Browser
    # fetch resolution precedes async crypto/decoder/React publication.
    eventually(lambda:w.sync("return document.readyState==='complete'"))
    return fixture


def wait_initial_pick(w,at):
    pick(w,at)
    # If the first click beat the provider publication, retry only at startup.
    def first_picked():
        status=w.sync("""
          const p=document.querySelector(arguments[0]);return {
            marker:!!document.querySelector('circle[aria-label="バスの起点"]'),
            alert:p?.querySelector('[role=alert]')?.textContent??''};""",PANEL)
        if status['marker']:
            return True
        if '道路を読み込み中' in status['alert']:
            pick(w,at);return False
        require(not status['alert'],f"ordinary map start refused: {status['alert']}")
        return False
    eventually(first_picked,timeout=10)


def run(w,scenario):
    fixture=initiate(w,scenario)
    primary=fixture["primaryShas"]
    first,last=fixture["anchors"]
    wait_initial_pick(w,first["geo"])
    pick(w,last["geo"])
    path=wait_visible(w,'path[aria-label="バス系統のプレビュー"]')
    preview=w.sync("return arguments[0].getAttribute('d')",path)
    require(preview.count("L")>=2,"native preview lacks the expected bend or seam")
    back=wait_visible(w,'path[aria-label="バス系統の復路プレビュー"]')
    back_d=w.sync("return arguments[0].getAttribute('d')",back)
    require(back_d.count("L")>=2 and preview!=back_d,"native return geometry missing or copied in the outward direction")
    require(w.sync("""
      const ends=p=>[p.getPointAtLength(0),p.getPointAtLength(p.getTotalLength())];
      const [a,b]=ends(arguments[0]),[c,d]=ends(arguments[1]);
      return Math.hypot(a.x-d.x,a.y-d.y)<=.01&&Math.hypot(b.x-c.x,b.y-c.y)<=.01;
    """,path,back),"native preview and return endpoints are not reversed")
    before=snapshot(w)
    require(not proofs(before) and not before.get('road_transport',{}).get('bus_routes'),"preview mutated native world")
    w.click_text("バス系統を確定")
    built_snapshot=eventually(lambda:(s if proofs(s:=snapshot(w)) else None))
    require(wait_visible(w,'path[aria-label="作成済みのバス経路"]'),"built route not rendered")
    road=built_snapshot['road_transport']
    require(len(road['bus_routes'])==1 and len(road['bus_stops'])==2,"incorrect native Bus entities")
    route=road['bus_routes'][0]
    indexed=proofs(built_snapshot)
    require(list(indexed)==[route['id']] and not built_snapshot.get('ofm_gameplay_bus'),"commit did not use the indexed route")
    binding=indexed[route['id']]
    require(binding.get('pattern')=='openRoundTrip',"native return pattern not retained")
    require(binding['rule']==("ofm_ground_bus_rectangle_z14_v2" if scenario=="seam" else "ofm_ground_bus_z14_v1"),f"unexpected bus reconstruction rule: {binding}")
    require(binding['tileKey']==first['key'] and binding.get('endTileKey',binding['tileKey'])==last['key'],"wrong route cell order")
    require(binding.get('respectOneway') is False,"default ignored-oneway policy absent")
    require(binding['tileRawSha256']==primary[first['key']],"native did not retain fixture start digest")
    require([binding[k]['vertex'] for k in ('start','end')]==[first['vertex'],last['vertex']],"native path endpoint mismatch")
    additional={p['tileKey']:p['tileRawSha256'] for p in binding.get('additionalTiles',[])}
    require(additional=={key:sha for key,sha in primary.items() if key!=first['key']},"native did not retain exact seam tile proof")
    verify_existing_edit(w,last,route['id'],binding)
    binding=stop_edit(w,route['id'],binding)
    bus_key,clock=drive_and_pause(w,route['id'])
    return save(w,binding,route['id'],bus_key,clock,STOP_NAME)


def save(w,binding,route_id,bus_key,clock,stop_name):
    w.click(eventually(lambda:w.element('css selector','#workspace-tab-saves')))
    w.click_text('ワールドを保存')
    eventually(lambda:w.sync("""return document.body.innerText.includes('ワールドと未着工計画を保存しました。')"""))
    slot=w.native('list_save_slots')
    selected=next(s for s in slot['slots'] if s['id']==slot['active_slot_id'])
    require(selected['loadable'],"native save did not create a loadable active slot")
    return selected['name'],binding,route_id,bus_key,clock,stop_name


def run_loop(w):
    fixture=initiate(w,"loop")
    a,b,c=fixture["anchors"]
    wait_initial_pick(w,a["geo"])
    preview_css='path[aria-label="バス系統のプレビュー"]'
    pick(w,b["geo"])
    first=wait_visible(w,preview_css)
    old=w.sync("return arguments[0].getAttribute('d')",first)
    pick(w,c["geo"])
    eventually(lambda:w.sync("const p=document.querySelector(arguments[0]);return p&&p.getAttribute('d')!==arguments[1];",preview_css,old))
    endpoint='circle[aria-label="バスの起点"]'
    x,y=w.sync(SCREEN_POINT,MAP,projection(a['geo']))
    require(w.sync("return document.elementFromPoint(Math.round(arguments[0]),Math.round(arguments[1]))?.matches(arguments[2]);",x,y,endpoint),"original start handle not hit")
    pick(w,a['geo'])
    wait_panel_text(w,'環状系統（片方向）')
    wait_visible(w,'circle[aria-label="バスの起終点"]')
    path=wait_visible(w,preview_css)
    require(w.sync("""
        const p=arguments[0], a=p.getPointAtLength(0), b=p.getPointAtLength(p.getTotalLength()), d=p.getAttribute('d');
        return (d.match(/L/g)||[]).length>=3 && Math.hypot(a.x-b.x,a.y-b.y)<=.01 &&
          !document.querySelector('path[aria-label="バス系統の復路プレビュー"]') &&
          document.querySelectorAll('circle[aria-label="バスの起終点"]').length===1;
        """,path),"preview is not a single real circuit")
    wait_panel_text(w,'中間停留所（自動）')
    require(not proofs(snapshot(w)),"closed-loop draft mutated native state")
    w.click_text("バス系統を確定")
    built=eventually(lambda:(s if proofs(s:=snapshot(w)) else None))
    roads=built['road_transport']
    require(len(roads['bus_routes'])==1 and len(roads['bus_stops'])==2,"native loop lost origin/fallback or invented a return stop")
    route=roads['bus_routes'][0]
    exact=proofs(built)[route['id']]
    require(exact['pattern']=='closedOneWay' and exact['rule']=='ofm_ground_bus_z14_v1' and not exact.get('respectOneway'),
            "not a native one-way circuit; policy must stay distinct from motor one-way restriction")
    vertex=lambda anchor:{"feature":anchor['feature'],"line":0,"vertex":anchor['vertex']}
    require(exact['start']==exact['end']==vertex(a),"native circuit did not close on exact start")
    require([(v['tileKey'],v['vertex']) for v in exact['via']]==[(b['key'],vertex(b)),(c['key'],vertex(c))],"authored point order changed")
    require(exact['tileKey']==a['key'] and exact['tileRawSha256']==fixture['primaryShas'][a['key']] and len(route['edges'])==3,"wrong wire-evidence/fake closure")
    require(not exact.get('interiorStops'),"fallback is not a saved user-authored stop")
    bus_key,clock=drive_and_pause(w,route['id'])
    return save(w,exact,route['id'],bus_key,clock,'中間停留所')


def reopen(w,expected):
    name,binding,route_id,bus_key,clock,stop_name=expected
    # The second process does not get the fetch shim/cache. Successful restore
    # therefore relies on its persisted raw evidence and native validation.
    w.click_text("保存済みワールドを読み込む")
    w.click(w.action_element(f"//button[@aria-label={json.dumps(name,ensure_ascii=False)}]"))
    restored=eventually(lambda:(s if route_id in proofs(s:=snapshot(w)) else None))
    require(proofs(restored)=={route_id:binding} and not restored.get('ofm_gameplay_bus'),"indexed proof changed across process and disk")
    require([r['id'] for r in restored['road_transport']['bus_routes']]==[route_id],"route identity drift")
    require(any(s['name']==stop_name for s in restored['road_transport']['bus_stops']),"named stop missing on reopen")
    op=eventually(lambda: (o if (o:=w.native('operation_snapshot')).get('bus_services') else None))
    require(op['paused'] and op['now_ms']==clock and bus_key in [t['key'] for t in op['trains']],"paused live Bus did not survive native reopen")
    open_bus(w)
    wait_visible(w,'path[aria-label="作成済みのバス経路"]')
    require(w.sync('return !window.__ordinaryBusNetworkSmoke'),"reopen reused WebView fixture shim")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',required=True,type=Path)
    p.add_argument('--driver',default='tauri-driver')
    p.add_argument('--logs',required=True,type=Path)
    p.add_argument('--scenario',choices=('single','seam','loop'),default='seam')
    opts=p.parse_args()
    require(opts.app.is_file() and os.access(opts.app,os.X_OK),"not an executable application")
    opts.logs.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rosenfu-ordinary-bus-fixture-') as root:
        with Driver(opts.driver,Path(root),opts.logs) as w:
            w.open(opts.app.resolve())
            expected=run_loop(w) if opts.scenario=="loop" else run(w,opts.scenario)
        with Driver(opts.driver,Path(root),opts.logs) as w:
            w.open(opts.app.resolve())
            reopen(w,expected)
    print(f"Actual Tauri/WebKit: ordinary Bus {opts.scenario}, native route, native run, and exact paused disk reopen passed with network-only synthetic {REVISION}. No live CDN claim.")

if __name__=='__main__': main()
