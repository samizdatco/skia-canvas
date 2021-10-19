import OSS from 'ali-oss'
import * as fs from 'fs'

const client = new OSS({
  region: 'oss-cn-beijing',
  accessKeyId: 'LTAI4FsJSM4YcazEKzPLAn99',
  accessKeySecret: 'WfVPoKubLjh1fM9P9fnhtaMlw8HnXs',
  bucket: 'mock-test'
})


client.list().then((result) => {
  console.log('objects: %j', result.objects);
  return client.put('my-obj', new OSS.Buffer('hello world'));
})

async function main(){

}