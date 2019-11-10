import { action, computed, observable } from 'mobx'

const baseUrl = process.env.VUE_APP_LITTLECI_HOST || ''

interface User {
  username: string,
  exp: string,
  token: string,
}

interface Config {
  data_dir: string,
  network_host: string,
  site_url: string,
  port: string,
  log_to_syslog: boolean,
}

interface ErrorResponse {
  message: string,
}

export default class State {
  @observable user: User | null = null

  @observable config: Config | null = null

  @computed get loggedIn() {
    return this.user !== null
  }

  @action.bound async login(username: string, password: string) {
    const response = await fetch(`${baseUrl}/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        username,
        password,
      }),
    })

    if (!response.ok) {
      const responseObject: ErrorResponse = await response.json()
      throw new Error(responseObject.message)
    }

    this.user = await response.json()
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

