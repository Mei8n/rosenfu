// @vitest-environment jsdom
import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import {cleanup} from "@testing-library/react";
import {invoke} from "@tauri-apps/api/core";
import {InstalledSeamBusPanel} from "./InstalledSeamBusPanel";

vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
const native=vi.mocked(invoke);
afterEach(()=>{cleanup();vi.clearAllMocks();});

describe("installed seam bus authoring",()=>{
  it("does not expose the control without backend-approved options",async()=>{
    native.mockResolvedValue({expectedWorldToken:"",options:[]});
    render(<InstalledSeamBusPanel scope={{}} enabled commit={vi.fn()}/>);
    await waitFor(()=>expect(native).toHaveBeenCalledWith("installed_seam_bus_options"));
    expect(screen.queryByText("確認済み区間に作成")).toBeNull();
  });
  it("binds selection to the preview token and shows an atomic backend refusal",async()=>{
    native.mockResolvedValue({expectedWorldToken:"t",options:[{catalogKey:"approved",routeId:"seam:approved",lengthM:120,
      points:[{latitude:35,longitude:139}]}]});
    const commit=vi.fn().mockRejectedValue(Error("ワールドが変更されました"));
    const scope={};
    const view=render(<InstalledSeamBusPanel scope={scope} enabled commit={commit}/>);
    fireEvent.change(await screen.findByLabelText("境界横断バス系統名"),{target:{value:"試験バス"}});
    fireEvent.click(screen.getByText("確認済み区間に作成"));
    expect((await screen.findByRole("alert")).textContent).toContain("ワールドが変更されました");
    expect(commit).toHaveBeenCalledWith({expectedWorldToken:"t",catalogKey:"approved",routeId:"seam:approved",name:"試験バス"});
    native.mockResolvedValue({expectedWorldToken:"",options:[]});
    view.rerender(<InstalledSeamBusPanel scope={{}} enabled commit={commit}/>);
    expect(screen.queryByText("確認済み区間に作成")).toBeNull();
  });
  it("offers acquisition only for an installed backend reference",async()=>{
    const missing=[{revision:"trusted",z:14,x:1,y:0,raw_sha256:"hash",decoder_rule:"rule"}];
    native.mockResolvedValue({expectedWorldToken:"before",options:[],acquisitions:[{catalogKey:"managed",missing}]});
    const acquire=vi.fn().mockRejectedValue(Error("道路の版が変わりました"));
    render(<InstalledSeamBusPanel scope={{}} enabled acquire={acquire} commit={vi.fn()}/>);
    expect(screen.queryByText("確認済み区間に作成")).toBeNull();
    fireEvent.click(await screen.findByText("確認済み区間の道路を取得"));
    expect((await screen.findByRole("alert")).textContent).toContain("道路の版が変わりました");
    expect(acquire).toHaveBeenCalledWith({expectedWorldToken:"before",catalogKey:"managed",missing});
  });
});
