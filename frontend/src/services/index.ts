// =============================================================================
// CTS API 클라이언트 (services/index.ts)
// =============================================================================
// axios 로 서버 REST API 호출. 기본 서버 주소는 VITE_API_URL 로 덮어쓸 수 있다.

import axios from 'axios'
import type {
  BlobContent,
  Branch,
  Build,
  Commit,
  Repository,
  TreeEntry,
} from '../types'

const API_BASE: string =
  (import.meta.env.VITE_API_URL as string | undefined) ?? 'http://127.0.0.1:8080'

const api = axios.create({ baseURL: `${API_BASE}/api` })

export const getRepositories = () =>
  api.get<Repository[]>('/repositories').then((r) => r.data)

export const getRepository = (id: string) =>
  api.get<Repository>(`/repositories/${id}`).then((r) => r.data)

export const getBranches = (id: string) =>
  api.get<Branch[]>(`/repositories/${id}/branches`).then((r) => r.data)

export const getCommits = (id: string, branch: string) =>
  api
    .get<Commit[]>(`/repositories/${id}/commits`, { params: { branch } })
    .then((r) => r.data)

export const getTree = (id: string, commit: string, path = '') =>
  api
    .get<TreeEntry[]>(`/repositories/${id}/tree/${commit}`, { params: { path } })
    .then((r) => r.data)

export const getBlob = (id: string, hash: string) =>
  api.get<BlobContent>(`/repositories/${id}/blob/${hash}`).then((r) => r.data)

export const getBuilds = (id: string) =>
  api.get<Build[]>(`/repositories/${id}/builds`).then((r) => r.data)

export const getBuildLog = (id: string, buildId: string) =>
  api
    .get<string>(`/repositories/${id}/builds/${buildId}/log`, {
      responseType: 'text',
    })
    .then((r) => r.data)

export const triggerBuild = (id: string, commitHash: string) =>
  api
    .post<Build>(`/repositories/${id}/builds`, { commit_hash: commitHash })
    .then((r) => r.data)
