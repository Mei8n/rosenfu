// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {ReferenceNameSuggestions} from "./ReferenceNameSuggestions";
import {parseReferencePack,createReferenceRepository} from "./referenceStations";
afterEach(cleanup);
const pack=parseReferencePack(JSON.stringify({format:"rosenfu-reference-stations",version:1,dataset:{id:"names",revision:"1",source:"local fixture",license:"test",bounds:{west:139,east:140,south:35,north:36}},stations:[{osmType:"node",osmId:"1",name:"参考中央",latitude:35.675,longitude:139.75,kind:"station"}]}));
test("suggestions never auto-select and distinguish missing coverage and data",()=>{
 const onChoose=vi.fn(),props={pack,repository:createReferenceRepository(pack),point:{latitude:35.675,longitude:139.75},disabled:false,onChoose};
 const {rerender}=render(<ReferenceNameSuggestions {...props}/>);expect(onChoose).not.toHaveBeenCalled();expect(screen.getByText(/約0m・直線距離/)).toBeTruthy();expect(screen.getByText(/候補の出典: local fixture/)).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"参考中央を新駅名入力へ使う"}));expect(onChoose).toHaveBeenCalledExactlyOnceWith("参考中央");
 rerender(<ReferenceNameSuggestions {...props} point={{latitude:34,longitude:139.75}}/>);expect(screen.getByText(/パック範囲外のため候補なし/)).toBeTruthy();
 rerender(<ReferenceNameSuggestions {...props} point={{latitude:35.9,longitude:139.75}}/>);expect(screen.getByText(/周辺に駅がないという意味ではありません/)).toBeTruthy();
 rerender(<ReferenceNameSuggestions {...props} pack={null} repository={null}/>);expect(screen.getByText(/参照駅パック未導入/)).toBeTruthy();expect(onChoose).toHaveBeenCalledTimes(1);
});
test("disabled candidates never call onChoose",()=>{
 const onChoose=vi.fn();render(<ReferenceNameSuggestions pack={pack} repository={createReferenceRepository(pack)} point={{latitude:35.675,longitude:139.75}} disabled={true} onChoose={onChoose}/>);const button=screen.getByRole("button",{name:"参考中央を新駅名入力へ使う"});expect((button as HTMLButtonElement).disabled).toBe(true);fireEvent.click(button);expect(onChoose).not.toHaveBeenCalled();
});
