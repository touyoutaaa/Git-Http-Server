# Git-Http-Server
A Git-Http-Server by Rusts

## 测试命令
1. 创建仓库
   测试创建新的Git仓库：

curl -X GET "http://localhost:3000/new_repo?name=test-repo" \
预期结果：返回"Repository created successfully"，并且在git_repo目录下创建一个名为test-repo的裸仓库。

2. 查询仓库引用信息
   测试获取仓库的引用信息：

curl -X GET "http://localhost:3000/my-repo/info/refs?service=git-upload-pack" --output - \
预期结果：返回二进制数据（Git协议数据）。虽然直接查看可能是乱码，但这表明服务器正在正确响应。

3. 克隆仓库
   测试克隆仓库：

git clone http://localhost:3000/test-repo test-local-repo \
预期结果：成功克隆仓库到test-local-repo目录。

### 进入克隆的仓库
cd test-local-repo

### 创建一个测试文件
echo "# Test Repository" > README.md

### 提交更改
git add README.md
git commit -m "Initial commit"

### 推送到服务器
git push origin master
预期结果：更改成功推送到服务器。

4. 再次克隆验证
   测试克隆已有内容的仓库，验证之前的推送是否成功：

git clone http://localhost:3000/test-repo test-verify-repo
然后检查克隆的仓库是否包含之前推送的内容：

cd test-verify-repo
cat README.md  # 应该显示 "# Test Repository"
5. 删除仓库
   测试删除仓库：

curl -X DELETE "http://localhost:3000/test-repo"
预期结果：返回"Repository deleted successfully"，并且git_repo目录下的test-repo目录被删除。






