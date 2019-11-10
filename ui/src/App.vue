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
import Component from 'vue-class-component'
import Login from './components/Login.vue'
import {state} from './store/state'

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

  items = [
    { icon: 'fas fa-home', text: 'Home', to: '/' },
    { divider: true },
    { text: 'Repo A', to: '/repo-a' },
    { text: 'Repo B', to: '/repo-b' },
    { divider: true },
    { icon: 'fas fa-cog', text: 'Config', to: '/config' },
    { icon: 'fas fa-globe-africa', text: 'API Docs', href: '/swagger/index.html' },
  ]
}
</script>

<style>
</style>
