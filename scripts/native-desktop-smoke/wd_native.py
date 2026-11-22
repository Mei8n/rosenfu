"""Local loopback only: W3C user actions and read-only Tauri diagnostics."""
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from urllib.error import HTTPError
from urllib.request import Request, build_opener, ProxyHandler

ELEMENT = "element-6066-11e4-a52e-4f735466cecf"
READ_ONLY = {"engine_info","world_snapshot","list_save_slots","list_native_road_regions","operation_snapshot"}

class DriverError(RuntimeError):
    pass

def require(ok, message):
    if not ok:
        raise AssertionError(message)

def eventually(f, timeout=8):
    end = time.monotonic()+timeout
    last = None
    while time.monotonic() < end:
        try:
            if value := f():
                return value
        except DriverError as exc:
            if not any(tag in str(exc) for tag in ('no such element', 'stale element reference')):
                raise
            last=exc
        time.sleep(.075)
    raise AssertionError(f"timeout: {last or getattr(f, '__name__', f)}")

def nav_click_payload(x, y):
    return {"actions":[{"type":"pointer","id":"map-mouse","parameters":{"pointerType":"mouse"},"actions":[
        {"type":"pointerMove","duration":0,"origin":"viewport","x":round(x),"y":round(y)},
        {"type":"pointerDown","button":0},{"type":"pointerUp","button":0}]}]}

class WD:
    def __init__(self, port):
        self.base=f"http://127.0.0.1:{port}"
        self.http=build_opener(ProxyHandler({}))
        self.session=None
    def rpc(self, method, path, obj=None):
        req=Request(self.base+path, data=None if obj is None else json.dumps(obj).encode(),
            headers={"Content-Type":"application/json"}, method=method)
        try:
            with self.http.open(req, timeout=15) as r:
                parsed=json.load(r)
        except HTTPError as e:
            raise DriverError(e.read().decode(errors="replace")) from e
        value=parsed.get("value")
        if isinstance(value,dict) and value.get("error"): raise DriverError(str(value))
        return value
    def command(self, method, path, obj=None):
        require(self.session, "WebDriver session not established")
        return self.rpc(method,f"/session/{self.session}"+path,obj)
    def open(self, binary):
        value=self.rpc("POST","/session",{"capabilities":{"alwaysMatch":{"tauri:options":{"application":str(binary)}}}})
        self.session=value["sessionId"]
        self.command("POST","/timeouts",{"implicit":0,"script":7000,"pageLoad":12000})
    def close(self):
        if self.session:
            self.command("DELETE","")
            self.session=None
    def sync(self, script, *args):
        return self.command("POST","/execute/sync",{"script":script,"args":args})
    def native(self, name):
        require(name in READ_ONLY, "Only audited read-only IPC may run from WebDriver script")
        result=self.command("POST","/execute/async",{"args":[name],"script":"""
            const [command,done]=arguments;
            const native=window.__TAURI_INTERNALS__;
            if(!native?.invoke){done({error:'missing native IPC'});return;}
            native.invoke(command).then(value=>done({value}),error=>done({error:String(error)}));
            """})
        require("value" in result, f"{name}: {result}")
        return result["value"]
    def element(self, strategy, selector):
        return self.command("POST","/element",{"using":strategy,"value":selector})
    def click(self, element):
        self.command("POST","/element/"+element[ELEMENT]+"/click",{})
    def text_action(self, text, tag="button"):
        expr=f"//{tag}[normalize-space(.)={json.dumps(text,ensure_ascii=False)}]"
        # DOM-based filtering finds only displayed, enabled controls across colliding panels.
        return self.action_element(expr)
    def action_element(self, xpath):
        return eventually(lambda: self.sync("""
            const result=document.evaluate(arguments[0],document,null,XPathResult.ORDERED_NODE_SNAPSHOT_TYPE);
            for(let i=0;i<result.snapshotLength;i++){
              const e=result.snapshotItem(i);if(e.getClientRects().length&&!e.matches(':disabled'))return e;
            } return null;""",xpath))
    def select(self, label, value):
        e=self.label_control(label,"select")
        opts=self.sync("return [...arguments[0].options].filter(o=>o.value===arguments[1])", e, value)
        require(len(opts)==1, f"option for {label} not found: {value}")
        self.click(opts[0])
        require(self.sync("return arguments[0].value",e)==value,f"selector didn't apply {label}")
    def label_control(self, label, control):
        return eventually(lambda: self.sync("""
          for(const label of document.querySelectorAll('label')){
            const c=label.control;
            if(c?.matches(arguments[1])&&c.getClientRects().length&&!c.disabled
              && [...label.childNodes].some(n=>n.nodeType===3&&n.textContent.trim()===arguments[0]))return c;
          }return null;""",label,control))
    def input_css(self, css, value):
        return self.input_element(eventually(lambda: self.element("css selector",css)),value)
    def input_element(self, element, value):
        e=element[ELEMENT]
        self.command("POST",f"/element/{e}/clear",{})
        self.command("POST",f"/element/{e}/value",{"text":value,"value":list(value)})
    def click_text(self, text, tag="button"):
        self.click(self.text_action(text,tag))
class Driver:
    def __init__(self, executable, root, logs):
        self.executable=executable
        self.root=root
        self.logs=logs
    def __enter__(self):
        require(os.environ.get("DISPLAY"), "Xvfb or supported X11 display required")
        with socket.socket() as front, socket.socket() as backend:
            for socket_ in (front, backend):
                socket_.bind(("127.0.0.1",0))
            port, native_port=(s.getsockname()[1] for s in (front, backend))
        env=os.environ.copy()
        for key, folder in [("XDG_DATA_HOME","data"),("XDG_CACHE_HOME","cache"),("XDG_CONFIG_HOME","config")]:
            target=self.root/folder; target.mkdir(exist_ok=True)
            env[key]=str(target)
        self.log=open(self.logs/"driver.log","a")
        env["HOME"]=str(self.root/"home")
        Path(env["HOME"]).mkdir(exist_ok=True)
        self.process=subprocess.Popen([self.executable,"--port",str(port),"--native-port",str(native_port)],env=env,stdout=self.log,stderr=subprocess.STDOUT)
        self.w=WD(port)
        try:
            # A loopback socket readiness check never navigates the WebView.
            def ready():
                if self.process.poll() is not None:
                    raise RuntimeError("tauri-driver exited; see driver.log")
                try:
                    with socket.create_connection(("127.0.0.1",port),.3):return True
                except OSError:return False
            eventually(ready)
            return self.w
        except BaseException:
            self.__exit__(None,None,None)
            raise
    def __exit__(self,*exc):
        try:
            self.w.close()
        finally:
            self.process.terminate()
            try:self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill();self.process.wait()
            self.log.close()
