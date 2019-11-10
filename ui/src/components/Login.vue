<template>
  <v-app>
    <v-content>
      <v-container
        class="fill-height"
        fluid
      >
        <v-row
          align="center"
          justify="center"
        >
          <v-col
            cols="12"
            sm="8"
            md="4"
          >
          <v-form @submit.prevent="onClick">
            <v-card class="elevation-12">
              <v-toolbar
                dark
                flat
              >
                <v-toolbar-title>Login</v-toolbar-title>
                <v-spacer />
              </v-toolbar>
              <v-card-text>
                  <v-text-field
                    label="Username"
                    name="Username"
                    prepend-icon="fas fa-user"
                    type="text"
                    v-model="username"
                    outlined
                  />

                  <v-text-field
                    id="password"
                    label="Password"
                    name="password"
                    prepend-icon="fas fa-lock"
                    type="password"
                    v-model="password"
                    outlined
                  />
              </v-card-text>
              <v-card-actions>
                <v-spacer />
                <v-btn type="submit" color="primary">Login</v-btn>
              </v-card-actions>
            </v-card>
          </v-form>
          </v-col>
        </v-row>
      </v-container>
    </v-content>
  </v-app>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state} from '../store/state'

@Component({
  name: 'Login',
})
export default class Login extends Vue {
  state = state

  username = ''
  password = ''

  async onClick() {
    await this.state.login(this.username, this.password)
  }
}
</script>

