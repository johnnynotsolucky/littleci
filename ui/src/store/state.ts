import { action, computed, observable } from 'mobx'

const baseUrl = process.env.VUE_APP_LITTLECI_HOST || ''

interface User {
  username: string,
  exp: string,
  token: string,
}

interface Config {
  config_path: string,
  data_dir: string,
  network_host: string,
  site_url: string,
  port: string,
  log_to_syslog: boolean,
}

export interface Repository {
  id?: string,
  slug?: string,
  name: string,
  run?: string,
  working_dir?: string,
  variables?: {},
  triggers?: [],
  secret?: string,
}

export interface Job {
  id: string,
  repository: string,
  status: string,
  exit_code: number,
  data: object,
  created_at: Date,
  updated_at: Date,
  logs: Log[],
}

export interface Log {
  status: string,
  exit_code: number,
  created_at: Date,
}

interface ErrorResponse {
  message: string,
}

const makeRequest = async (url: string, options: object): Promise<Response> => {
    const response = await fetch(url, options)

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    return response
}

export default class State {
  @observable repositories: Repository[] = []
  @observable user: User | null = null
  @observable config: Config | null = null

  @computed get loggedIn() {
    return this.user !== null
  }

  @action.bound async login(username: string, password: string) {
    const response = await makeRequest(`${baseUrl}/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        username,
        password,
      }),
    })

    this.user = await response.json()
  }

  @action.bound async saveNewRepository(repository: Repository): Promise<Repository> {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await makeRequest(`${baseUrl}/repositories`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
      body: JSON.stringify(repository),
    })

    const result = await response.json()
    await this.getRepositories()

    return result
  }

  @action.bound async getRepositories() {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await fetch(`${baseUrl}/repositories`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    this.repositories = await response.json()
  }

  @action.bound async getRepository(slug: string): Promise<Repository> {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await fetch(`${baseUrl}/repositories/${slug}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    return await response.json()
  }

  @action.bound async getRepositoryJobs(repository: string): Promise<Job[]> {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await fetch(`${baseUrl}/repositories/${repository}/jobs`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    return await response.json()
  }

  @action.bound async getJob(repository: string, jobId: string): Promise<Job> {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await fetch(`${baseUrl}/repositories/${repository}/jobs/${jobId}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    return await response.json()
  }

  @action.bound async getJobOutput(repository: string, jobId: string): Promise<string> {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await fetch(`${baseUrl}/repositories/${repository}/jobs/${jobId}/output`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    return await response.text()
  }

  @action.bound async loadConfig() {
    if (!this.user) {
      throw new Error('Not logged in')
    }

    const response = await fetch(`${baseUrl}/config`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.user.token}`,
      },
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    this.config = await response.json()
  }
}

export const state = new State()

