import './EditingMapFrame.css';

type Props = {
  enabled: boolean;
  tab: string;
  tool: string;
  mapMode: 'move' | 'place' | 'draw';
  pickingStations: boolean;
};

// Derive from the active interaction, never from an unsaved draft or selection.
// Moving the camera temporarily does not end an editing mode.
export function editingMapMode({ enabled, tab, tool, mapMode, pickingStations }: Props) {
  if (!enabled) return null;
  if (pickingStations && tab === 'routes') return { key: 'route', label: '路線の駅を選択中' };
  if (pickingStations && tab === 'services') return { key: 'service', label: '系統の駅を選択中' };
  if (tab !== 'construction') return null;
  if (tool === 'tracks' && mapMode === 'draw') return { key: 'track', label: '線路敷設モード' };
  if (tool === 'stations' && mapMode === 'place') return { key: 'station', label: '駅設置モード' };
  if (tool === 'delete') return { key: 'delete', label: '撤去モード' };
  if (tool === 'select') return { key: 'select', label: '範囲選択モード' };
  return null;
}

export function EditingMapFrame(props: Props) {
  const mode = editingMapMode(props);
  return <div className="editing-map-frame" data-mode={mode?.key}>
    {/* Keep the live region mounted so entering/exiting modes is announced.
        Visible tab/tool labels remain the non-colour cue. */}
    <span className="editing-map-announcement" role="status">{mode?.label ?? ''}</span>
  </div>;
}
