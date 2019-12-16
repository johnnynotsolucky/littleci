<template>
  <v-card flat style="border-radius: 0;">
    <v-toolbar
      flat
      dark
    >
      <v-toolbar-title>
        Add New Repository
      </v-toolbar-title>
      <v-spacer />
      <v-toolbar-items>
        <v-btn @click="onSave">
          Save
          <v-icon right small>fas fa-save</v-icon>
        </v-btn>
      </v-toolbar-items>
    </v-toolbar>
    <v-container
      fluid
      class="grey lighten-4 fill-height"
    >
      <v-row>
        <v-col>
          <v-text-field
            v-model="repositoryName"
            label="Name"
            outlined
          ></v-text-field>
        </v-col>
      </v-row>
      <v-row>
        <v-col>
          <v-text-field
            v-model="runCommand"
            label="Run Command"
            outlined
          ></v-text-field>
        </v-col>
      </v-row>
      <v-row>
        <v-col>
          <v-text-field
            v-model="workingDirectory"
            label="Working Directory"
            outlined
          ></v-text-field>
        </v-col>
      </v-row>
    </v-container>
  </v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state} from '../store/state'

@Component({
  name: 'NewRepository',
})
export default class NewRepository extends Vue {
  state = state

  repositoryName = ''
  runCommand = ''
  workingDirectory = ''

  async onSave() {
    await this.state.saveNewRepository({
      name: this.repositoryName,
      run: this.runCommand,
      working_dir: this.workingDirectory,
    })
  }
}
</script>
