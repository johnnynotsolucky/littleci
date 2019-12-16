<template>
  <Login v-if="!state.loggedIn" />
  <v-app id="keep" v-else>
    <v-app-bar
      app
      clipped-left
    >
      <v-app-bar-nav-icon @click="drawer = !drawer" />
      <!-- <v-img -->
      <!--   alt="Vuetify Logo" -->
      <!--   class="shrink mr-2" -->
      <!--   contain -->
      <!--   src="./assets/logo.svg" -->
      <!--   transition="scale-transition" -->
      <!--   width="40" -->
      <!-- /> -->
      <span class="title ml-3 mr-5">LittleCI</span>
      <v-spacer />
    </v-app-bar>

    <v-navigation-drawer
      v-model="drawer"
      app
      clipped
      color="grey lighten-4"
    >
      <v-list
        dense
      >
        <template v-for="(item, i) in items">
          <v-row
            v-if="item.heading"
            :key="i"
          >
            <v-col cols="6">
              <v-subheader v-if="item.heading">
                {{ item.heading }}
              </v-subheader>
            </v-col>
          </v-row>
          <v-divider
            v-else-if="item.divider"
            :key="i"
            dark
            class="my-4"
          />
          <v-list-item
            v-else
            :key="i"
            link
            :to="item.to"
            :href="item.href"
          >
            <v-list-item-action v-if="item.icon">
              <v-icon>{{ item.icon }}</v-icon>
            </v-list-item-action>
            <v-list-item-content>
              <v-list-item-title class="grey--text">
                {{ item.text }}
              </v-list-item-title>
            </v-list-item-content>
          </v-list-item>
        </template>
      </v-list>
    </v-navigation-drawer>

    <v-content>
      <router-view />
    </v-content>
  </v-app>
</template>

<script lang="ts">
import Vue from 'vue'
import { Component, Watch } from 'vue-property-decorator'
import Login from './components/Login.vue'
import {state, Repository} from './store/state'

@Component({
  props: {
    source: String,
  },
  components: {
    Login,
  },
})
export default class App extends Vue {
  state = state
  drawer = null
  repositories: Repository[] = []

  get items() {
    return [
      ...this.mappedRepositories,
      { icon: 'fas fa-tasks', text: 'Manage Repositories', to: '/manage/repositories' },
      { divider: true },
      { icon: 'fas fa-users', text: 'Users', to: '/manage/users' },
      { icon: 'fas fa-cog', text: 'Config', to: '/config' },
      { icon: 'fas fa-question-circle', text: 'API Docs', href: '/swagger/index.html' },
    ]
  }

  get mappedRepositories() {
    return this.state.repositories.map((repository) => ({
      text: repository.name,
      to : `/repositories/${repository.slug}`,
    }))
  }

  @Watch('state.loggedIn', { immediate: true })
  async onLoggedInChanged(loggedIn: boolean) {
    if (loggedIn) {
      await this.state.getRepositories()
    }
  }
}
</script>

<style>
</style>
