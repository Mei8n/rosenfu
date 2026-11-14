// @vitest-environment jsdom
import React from "react";
import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,test} from "vitest";
import {MapCredits} from "./MapCredits";
afterEach(cleanup);
test("OSM tile credit stays outside collapsed disclosure and is not duplicated by overlays",()=>{
 const {container}=render(<MapCredits background="osm" reference={{source:"OSM",license:"ODbL"}} graphVisible/>);
 const link=screen.getByRole("link",{name:"© OpenStreetMap contributors"});
 expect(link.closest("details")).toBeNull();
 expect(container.querySelector("details")?.open).toBe(false);
 expect(screen.getAllByText("© OpenStreetMap contributors")).toHaveLength(1);
 expect(screen.getByLabelText("地図の出典と詳細")).toBeTruthy();
});
test("GSI background and OSM reference credits are both retained",()=>{
 render(<MapCredits background="gsi" reference={null} graphVisible/>);
 expect(screen.getByRole("link",{name:/国土地理院/})).toBeTruthy();
 expect(screen.getByRole("link",{name:"© OpenStreetMap contributors"})).toBeTruthy();
});
test("sample has no third-party background credit and no false OpenFreeMap credit",()=>{
 const {container}=render(<MapCredits background="sample" reference={null} graphVisible={false}/>);
 expect(container.querySelectorAll("a")).toHaveLength(0);
 expect(container.textContent).not.toContain("OpenFreeMap");
});
