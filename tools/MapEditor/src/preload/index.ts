import { contextBridge, ipcRenderer } from 'electron'
import type { IpcApi, AssetCategory, AssetCatalog, MapManifest, HarvestNodePlacement, WorldBounds } from '../shared/types'

const api: IpcApi = {
  // Maps directory
  getMapsDir:         ()                             => ipcRenderer.invoke('get-maps-dir'),
  listMaps:           ()                             => ipcRenderer.invoke('list-maps'),
  createMapInDir:     (name, bounds)                 => ipcRenderer.invoke('create-map-in-dir', name, bounds),

  // Legacy / external maps
  openMapDialog:      ()                             => ipcRenderer.invoke('open-map-dialog'),
  initMap:            (mapPath, name)                => ipcRenderer.invoke('init-map', mapPath, name),
  loadManifest:       (mapPath)                      => ipcRenderer.invoke('load-manifest', mapPath),
  saveManifest:       (mapPath, manifest)            => ipcRenderer.invoke('save-manifest', mapPath, manifest),

  // Chunks
  loadChunk:          (mapPath, layer, cx, cy)       => ipcRenderer.invoke('load-chunk', mapPath, layer, cx, cy),
  saveChunk:          (mapPath, layer, cx, cy, data) => ipcRenderer.invoke('save-chunk', mapPath, layer, cx, cy, data),
  listChunks:         (mapPath, layer)               => ipcRenderer.invoke('list-chunks', mapPath, layer),

  // Harvest nodes
  loadHarvestNodes:   (mapPath)                      => ipcRenderer.invoke('load-harvest-nodes', mapPath),
  saveHarvestNodes:   (mapPath, nodes)               => ipcRenderer.invoke('save-harvest-nodes', mapPath, nodes),

  // Asset library
  loadAllCatalogs:      ()                             => ipcRenderer.invoke('load-all-catalogs'),
  saveCatalog:          (catalog)                      => ipcRenderer.invoke('save-catalog', catalog),
  getAssetDataUrl:      (category, file)               => ipcRenderer.invoke('get-asset-data-url', category, file),
  loadHarvestNodeDefs:         ()                      => ipcRenderer.invoke('load-harvest-node-defs'),
  getGlbDataUrl:               (path)                  => ipcRenderer.invoke('get-glb-data-url', path),
  refreshAssets:               ()                      => ipcRenderer.invoke('refresh-assets'),
}

contextBridge.exposeInMainWorld('api', api)
